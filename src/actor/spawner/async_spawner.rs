use std::{future::Future, sync::Arc, time::Duration};

use crate::{Actor, DynResult};

use super::{ActorHandle, JoinFuture, Spawner};

#[derive(Copy, Clone, Debug, Default)]
pub struct AsyncStdSpawner;

impl<A: Actor> Spawner<A> for AsyncStdSpawner {
    fn spawn_actor<F>(future: F) -> Box<dyn ActorHandle<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static,
    {
        let handle = Arc::new(async_lock::Mutex::new(Some(async_std::task::spawn(future))));
        Box::new(move || -> JoinFuture<A> {
            let handle = Arc::clone(&handle);
            Box::pin(async move {
                let mut handle: Option<async_std::task::JoinHandle<DynResult<A>>> =
                    handle.lock().await.take();

                if let Some(handle) = handle.take() {
                    // TODO: don't eat the error
                    handle.await.ok()
                } else {
                    None
                }
            })
        })
    }

    fn spawn_future<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        async_std::task::spawn(future);
    }

    async fn sleep(duration: Duration) {
        async_std::task::sleep(duration).await;
    }
}
