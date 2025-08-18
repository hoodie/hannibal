//! Runtime utilities.
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
#[cfg(all(test, feature = "tokio_runtime"))]
pub use tokio::spawn;
#[cfg(feature = "tokio_runtime")]
pub use tokio::time::sleep;

/// Block on the given future.
///
/// This function is a convenience wrapper around a specific runtime's `block_on` function.
/// You do not necessarily need to use this function, it just makes testing and examples easier.
#[cfg(all(not(feature = "tokio_runtime"), feature = "async_runtime"))]
pub use async_global_executor::block_on;
#[cfg(all(test, not(feature = "tokio_runtime"), feature = "async_runtime"))]

pub use async_global_executor::spawn;

/// Sleep for the given duration.
///
/// This function is a convenience wrapper around a specific runtime's sleep function.
/// You do not necessarily need to use this function, it just makes testing and examples easier.
#[cfg(all(not(feature = "tokio_runtime"), feature = "async_runtime"))]
pub async fn sleep(duration: std::time::Duration) {
    async_io::Timer::after(duration).await;
}
