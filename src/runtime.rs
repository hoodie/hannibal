/// Block on the given future.
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


#[cfg(feature = "smol_runtime")]
pub use smol::block_on;

#[cfg(feature = "smol_runtime")]
pub async fn sleep(duration: std::time::Duration) {
    smol::Timer::after(duration).await;
}
