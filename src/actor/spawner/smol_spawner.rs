use std::{future::Future, sync::Arc, time::Duration};

use crate::{Actor, DynResult};

use super::{ActorHandle, JoinFuture, Spawner};

#[derive(Copy, Clone, Debug, Default)]
pub struct SmolSpawner;

impl<A: Actor> Spawner<A> for SmolSpawner {
    fn spawn_actor<F>(future: F) -> Box<dyn ActorHandle<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static,
    {
        let handle = Arc::new(async_lock::Mutex::new(Some(smol::spawn(future))));
        Box::new(move || -> JoinFuture<A> {
            let handle = Arc::clone(&handle);
            Box::pin(async move {
                log::trace!("spawning smol task");
                let mut handle: Option<smol::Task<DynResult<A>>> = handle.lock().await.take();

                if let Some(handle) = handle.take() {
                    // TODO: don't eat the error

                    let actor = handle.await.ok();
                    log::trace!("smol task completed");
                    return actor
                } else {
                    log::warn!("smol task already completed");
                    None
                }
            })
        })
    }

    fn spawn_future<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        smol::spawn(future).detach();
    }

    async fn sleep(duration: Duration) {
        smol::Timer::after(duration).await;
    }
}
