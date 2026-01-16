//! Runtime utilities.
//!
//! This module provides re-exports of `block_on`, `sleep`, and `spawn` functions from the runtime crates.
//!
//! This module provides re-exports of the `block_on` and `sleep` functions from the runtime crates.
//!

/// Block on the given future.
///
/// This function is a convenience wrapper around a specific runtime's `block_on` function.
/// You do not necessarily need to use this function, it just makes testing and examples easier.
///
/// # Panics
/// This jives up if `tokio` runtime is not enabled.
#[cfg(all(feature = "tokio_runtime", not(feature = "async_runtime")))]
pub fn block_on<F, M>(future: F) -> M
where
    F: Future<Output = M>,
{
    #[allow(clippy::unwrap_used)]
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(future)
}
#[cfg(all(not(feature = "tokio_runtime"), feature = "async_runtime"))]
use std::convert::Infallible;

#[cfg(feature = "tokio_runtime")]
pub use tokio::time::sleep;

#[cfg(feature = "tokio_runtime")]
pub use tokio::spawn;

/// Block on the given future.
///
/// This function is a convenience wrapper around a specific runtime's `block_on` function.
/// You do not necessarily need to use this function, it just makes testing and examples easier.
#[cfg(all(not(feature = "tokio_runtime"), feature = "async_runtime"))]
pub use async_global_executor::block_on;

/// Wrapper around `async_global_executor::spawn` that returns a future that resolves to the result of the spawned future.
#[cfg(all(not(feature = "tokio_runtime"), feature = "async_runtime"))]
pub async fn spawn<F: Future<Output = T> + Send + 'static, T: Send + 'static>(
    future: F,
) -> Result<T, Infallible> {
    Ok(async_global_executor::spawn(future).await)
}

/// Sleep for the given duration.
///
/// This function is a convenience wrapper around a specific runtime's sleep function.
/// You do not necessarily need to use this function, it just makes testing and examples easier.
#[cfg(all(not(feature = "tokio_runtime"), feature = "async_runtime"))]
pub async fn sleep(duration: std::time::Duration) {
    async_io::Timer::after(duration).await;
}
