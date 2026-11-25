//! One-shot backgrounded work delivered on the GTK main loop.
//!
//! The app runs blocking system calls (nix queries, config reads, activation)
//! off the GTK thread. Many call sites duplicated the same skeleton: spawn a
//! worker that computes a plain `T` and sends it on an mpsc channel, then poll
//! with `glib::timeout_add_local` until the value (or a disconnect) arrives.
//! `run_on_main` owns that skeleton so the "GTK never blocks and never crosses
//! the thread boundary" invariant lives in one place.

use std::time::Duration;

/// Run `blocking` on a worker thread and hand its result to `on_done` on the
/// GTK main loop, polling every `interval`. `on_done` receives `Ok(value)` on
/// a normal completion and `Err(())` if the worker died without sending, so
/// the caller can surface a failure instead of stalling on "Running…".
pub fn run_on_main<T: Send + 'static>(
    interval: Duration,
    blocking: impl FnOnce() -> T + Send + 'static,
    on_done: impl Fn(Result<T, ()>) + 'static,
) {
    let (tx, rx) = std::sync::mpsc::channel::<T>();
    std::thread::spawn(move || {
        let result = blocking();
        let _ = tx.send(result);
    });
    glib::timeout_add_local(interval, move || match rx.try_recv() {
        Ok(result) => {
            on_done(Ok(result));
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            on_done(Err(()));
            glib::ControlFlow::Break
        }
    });
}
