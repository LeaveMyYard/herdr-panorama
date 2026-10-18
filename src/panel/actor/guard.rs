//! A panic in one of the herdr side's tasks (Python's workers caught everything into an
//! error): caught and reported, so the panel keeps its terminal and a switch never stays
//! "running". The panic hook asks [`in_task`] and leaves such a panic to the task.

use std::cell::Cell;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::task::{Context, Poll};

thread_local! {
    static IN_TASK: Cell<bool> = const { Cell::new(false) };
}

/// Whether a guarded task is running on this thread right now.
pub fn in_task() -> bool {
    IN_TASK.with(Cell::get)
}

/// A future whose panic comes out as `Err(what)`.
pub struct Guarded<T> {
    fut: Pin<Box<dyn Future<Output = T> + Send>>,
}

/// Guards `fut`.
pub fn guarded<T>(fut: impl Future<Output = T> + Send + 'static) -> Guarded<T> {
    Guarded { fut: Box::pin(fut) }
}

impl<T> Future for Guarded<T> {
    type Output = Result<T, String>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let was = IN_TASK.with(|t| t.replace(true));
        let polled = catch_unwind(AssertUnwindSafe(|| self.fut.as_mut().poll(cx)));
        IN_TASK.with(|t| t.set(was));
        match polled {
            Ok(Poll::Ready(v)) => Poll::Ready(Ok(v)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(p) => Poll::Ready(Err(p
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".to_owned()))),
        }
    }
}

#[cfg(test)]
mod tests;
