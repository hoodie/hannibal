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
#[cfg(feature = "tokio_runtime")]
pub fn block_on<F, M>(future: F) -> M
where
    F: Future<Output = M>,
{
    #[allow(clippy::unwrap_used)]
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(future)
}
#[cfg(feature = "tokio_runtime")]
pub use tokio::time::sleep;

#[cfg(feature = "async_runtime")]
pub use async_std::task::{block_on, sleep};

/// Block on the given future.
///
/// This function is a convenience wrapper around a specific runtime's `block_on` function.
/// You do not necessarily need to use this function, it just makes testing and examples easier.
#[cfg(feature = "smol_runtime")]
pub use smol::block_on;

/// Sleep for the given duration.
///
/// This function is a convenience wrapper around a specific runtime's sleep function.
/// You do not necessarily need to use this function, it just makes testing and examples easier.
#[cfg(feature = "smol_runtime")]
pub async fn sleep(duration: std::time::Duration) {
    smol::Timer::after(duration).await;
}
