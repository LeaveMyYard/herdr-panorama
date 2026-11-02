//! One worker, a bounded queue and a bounded cache. The UI never waits for Codex.
use super::{Observation, Options, history, parse};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::time::{Duration, Instant};
const CAPACITY: usize = 32;
const CACHE_SIZE: usize = 128;
struct Entry {
    value: Option<Observation>,
    refreshed: Instant,
    pending: bool,
}
type Reply = (String, Option<Observation>);

pub struct Observer {
    options: Options,
    cache: HashMap<String, Entry>,
    requests: Option<SyncSender<String>>,
    replies: Receiver<Reply>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Observer {
    pub fn new(options: Options) -> Self {
        let (send, replies) = mpsc::sync_channel(CAPACITY);
        drop(send);
        Self {
            options,
            cache: HashMap::new(),
            requests: None,
            replies,
            stop: Arc::default(),
            worker: None,
        }
    }
    fn start(&mut self) {
        if self.requests.is_some() {
            return;
        }
        let (requests, receive) = mpsc::sync_channel::<String>(CAPACITY);
        let (send, replies) = mpsc::sync_channel(CAPACITY);
        let options = self.options.clone();
        let stop = self.stop.clone();
        let worker = std::thread::Builder::new()
            .name("codex-history".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                let mut unsupported = std::collections::HashSet::new();
                while let Ok(id) = receive.recv() {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let value = runtime.as_ref().ok().and_then(|rt| {
                        rt.block_on(history::fetch_cancellable(
                            &options,
                            &id,
                            &mut unsupported,
                            &stop,
                        ))
                        .ok()
                    });
                    if send.send((id, value)).is_err() {
                        break;
                    }
                }
            });
        if let Ok(worker) = worker {
            self.worker = Some(worker);
            self.requests = Some(requests);
            self.replies = replies;
        }
    }
    /// Schedule a refresh if needed and return immediately. None means pending or
    /// unavailable; use is_pending to distinguish those for a CLI-only warmup loop.
    pub fn observe(&mut self, exact_thread_id: &str) -> Option<Observation> {
        self.drain();
        if !parse::valid_id(exact_thread_id) {
            return None;
        }
        let now = Instant::now();
        if let Some(entry) = self.cache.get(exact_thread_id) {
            let interval = self.options.refresh_interval.max(if entry.value.is_some() {
                Duration::from_secs(1)
            } else {
                Duration::from_secs(10)
            });
            if entry.pending || now.duration_since(entry.refreshed) < interval {
                return entry.value.clone();
            }
        }
        if self.cache.len() >= CACHE_SIZE && !self.cache.contains_key(exact_thread_id) {
            let oldest = self
                .cache
                .iter()
                .filter(|(_, e)| !e.pending)
                .min_by_key(|(_, e)| e.refreshed)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                self.cache.remove(&id);
            } else {
                return None;
            }
        }
        self.start();
        let sent = self
            .requests
            .as_ref()
            .is_some_and(|tx| tx.try_send(exact_thread_id.to_owned()).is_ok());
        if sent {
            self.cache
                .entry(exact_thread_id.to_owned())
                .and_modify(|e| e.pending = true)
                .or_insert(Entry {
                    value: None,
                    refreshed: now,
                    pending: true,
                });
        }
        self.cache
            .get(exact_thread_id)
            .and_then(|e| e.value.clone())
    }
    fn drain(&mut self) {
        while let Ok((id, value)) = self.replies.try_recv() {
            if let Some(entry) = self.cache.get_mut(&id) {
                *entry = Entry {
                    value,
                    refreshed: Instant::now(),
                    pending: false,
                };
            }
        }
    }
    /// Results are drained by observe. A failed refresh is not pending and is retried
    /// after at least ten seconds. Queue-full IDs can be retried on the next observe.
    pub fn is_pending(&self, exact_thread_id: &str) -> bool {
        self.cache.get(exact_thread_id).is_some_and(|e| e.pending)
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.requests.take();
        // Disconnect before joining: release a worker waiting to send a result.
        let (_, empty) = mpsc::sync_channel(CAPACITY);
        self.replies = empty;
        // Cancellation interrupts the read, kills and reaps only our subprocess.
        // Finish before a one-shot CLI calls process::exit (which skips destructors).
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
