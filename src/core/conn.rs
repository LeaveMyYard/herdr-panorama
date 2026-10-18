//! A connection's two tasks (Python `Core._read` and `_write`): the reader holds the hello
//! deadline and hands each message to [`super::handle`]; the writer starts at the welcome and
//! drops a viewer whose send does not finish in time.

use super::handle::{Next, handle};
use super::viewer::{Role, Viewer};
use super::{Event, Shared, Verify};
use crate::proto::framing::LineReader;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::OwnedWriteHalf;

/// Serves one accepted connection until it ends, then drops the viewer.
pub async fn serve(
    stream: UnixStream,
    v: Arc<Viewer>,
    shared: Arc<Shared>,
    verify: Arc<impl Verify>,
) {
    let (mut rd, wr) = stream.into_split();
    let mut wr = Some(wr);
    let mut reader = LineReader::default();
    let mut buf = vec![0u8; 65536];
    let deadline = tokio::time::Instant::now() + shared.limits.hello;
    let mut writer = None;
    loop {
        // only the hello has a deadline: a quiet viewer is fine after it
        let until = if v.hello() { None } else { Some(deadline) };
        let read = tokio::select! {
            r = rd.read(&mut buf) => r,
            () = v.closed() => break,
            () = sleep_until(until) => {
                let _ = shared.events.send(Event::Note(format!("viewer dropped: no hello in {:?}", shared.limits.hello)));
                break;
            }
        };
        let n = match read {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let msgs = match reader.feed(&buf[..n]) {
            Ok(m) => m,
            Err(e) => {
                let _ = shared
                    .events
                    .send(Event::Note(format!("viewer dropped: {e}")));
                break;
            }
        };
        let mut seen = vec![];
        let mut end = false;
        for msg in msgs {
            match handle(&v, msg, &mut seen, verify.as_ref(), &shared.events).await {
                Next::Go => {}
                Next::Close => {
                    end = true;
                    break;
                }
                Next::Refuse(line) => {
                    // said directly, before the close
                    if let Some(w) = wr.as_mut() {
                        let _ = tokio::time::timeout(shared.limits.send, w.write_all(&line)).await;
                    }
                    end = true;
                    break;
                }
                Next::Welcomed => {
                    welcome(&v, &shared);
                    if let Some(w) = wr.take() {
                        writer = Some(tokio::spawn(write(w, v.clone(), shared.clone())));
                    }
                }
            }
        }
        if !seen.is_empty() {
            let mut once = vec![];
            for id in seen {
                if !once.contains(&id) {
                    once.push(id);
                }
            }
            let _ = shared.events.send(Event::Seen(once)); // one event per read
        }
        if end {
            break;
        }
    }
    drop_viewer(&v, &shared);
    if let Some(w) = writer {
        let _ = w.await;
    }
}

/// The welcome, then the last state for a panel.
fn welcome(v: &Viewer, shared: &Shared) {
    let line = crate::proto::framing::encode(
        &serde_json::json!({"welcome": {"protocol": crate::proto::PROTOCOL}}),
    );
    v.send(Arc::new(line.expect("a short line")));
    if v.role() == Role::Panel
        && let Some(last) = shared.last.lock().unwrap().clone()
    {
        v.send(last);
    }
}

/// Writes the outbox until the viewer closes; a send that does not finish in time drops it.
async fn write(mut wr: OwnedWriteHalf, v: Arc<Viewer>, shared: Arc<Shared>) {
    loop {
        while let Some(line) = v.next() {
            match tokio::time::timeout(shared.limits.send, wr.write_all(&line)).await {
                Ok(Ok(())) => {}
                _ => {
                    drop_viewer(&v, &shared);
                    return;
                }
            }
        }
        tokio::select! {
            () = v.wake.notified() => {}
            () = v.closed() => break,
        }
    }
    let _ = wr.shutdown().await;
}

fn drop_viewer(v: &Arc<Viewer>, shared: &Shared) {
    v.close();
    shared
        .viewers
        .lock()
        .unwrap()
        .retain(|x| !Arc::ptr_eq(x, v));
}

async fn sleep_until(at: Option<tokio::time::Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}
