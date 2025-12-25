pub mod buffers;
pub mod image_data;
pub mod math;

use std::sync::mpsc;

// We're spawning asynchronous task and using a channel to force
// synchronous behavior and to prevent coloring the function, as
// it will eventually end up in a wayland Dispatch callback which
// is not async. We also can't use block_on as those can cause
// deadlocks if used in async runtime
pub fn wait<F, Fut, T>(f: F) -> T
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel();

    tokio::spawn(async move {
        let result = f().await;
        tx.send(result).unwrap();
    });

    rx.recv().unwrap()
}
