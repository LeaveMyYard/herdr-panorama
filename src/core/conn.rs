//! A connection's two tasks (Python `Core._read` and `_write`): the reader holds the hello
//! deadline and hands each message to [`super::handle`]; the writer starts at the welcome and
//! drops a viewer whose send does not finish in time.

use super::handle::{Next, emit, handle};
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
                shared.note(format!("viewer dropped: no hello in {:?}", shared.limits.hello));
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
                shared.note(format!("viewer dropped: {e}"));
                break;
            }
        };
        let mut seen = vec![];
        let mut end = false;
        for msg in msgs {
            // a later hello's pane check gets a deadline of its own
            let verify_by = if v.hello() {
                tokio::time::Instant::now() + shared.limits.hello
            } else {
                deadline
            };
            match handle(
                &v,
                msg,
                &mut seen,
                verify.as_ref(),
                verify_by,
                &shared.events,
            )
            .await
            {
                Next::Go => {}
                Next::Close => {
                    end = true;
                    break;
                }
                Next::Refuse(line) => {
                    // said before the close: directly, or by the writer once it owns the socket
                    match wr.as_mut() {
                        Some(w) => {
                            let _ =
                                tokio::time::timeout(shared.limits.send, w.write_all(&line)).await;
                        }
                        None => v.finish(Arc::new(line)),
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
            let (once, over) = super::seen::once_each(seen, super::seen::MAX_SEEN);
            if over {
                shared.note(format!(
                    "a viewer's seen list was cut to {}",
                    super::seen::MAX_SEEN
                ));
            }
            emit(&v, &shared.events, Event::Seen(once)).await; // one event per read
        }
        if end {
            break;
        }
    }
    if v.finishing()
        && let Some(w) = writer.as_mut()
    {
        let _ = tokio::time::timeout(shared.limits.send, w).await; // it writes the last line
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
    if v.role() != Role::Panel {
        return;
    }
    if shared.shut.load(std::sync::atomic::Ordering::SeqCst) {
        v.tell_to_go(super::shutdown_line()); // after shutdown: nothing but the word to go
    } else if let Some(last) = shared.last.lock().unwrap().clone() {
        v.send(last);
    }
}

/// Writes the outbox until the viewer closes; a send that does not finish in time drops it.
/// A close cuts a write short, so the socket goes at once (Python's `shutdown(SHUT_RDWR)`).
async fn write(mut wr: OwnedWriteHalf, v: Arc<Viewer>, shared: Arc<Shared>) {
    loop {
        // read before the drain: a finish queued during it is written on the next round
        let last = v.finishing();
        while let Some(line) = v.next() {
            let sent = tokio::select! {
                r = tokio::time::timeout(shared.limits.send, wr.write_all(&line)) => matches!(r, Ok(Ok(()))),
                () = v.closed() => false,
            };
            if !sent {
                drop_viewer(&v, &shared);
                return;
            }
        }
        if last {
            break; // its last line is written
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
