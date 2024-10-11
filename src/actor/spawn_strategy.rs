use std::{future::Future, pin::Pin, sync::Arc};

use crate::Addr;

use super::{Actor, DynResult};

pub type JoinFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

pub type DynJoiner<A> = Box<dyn Joiner<A>>;
pub trait Joiner<A: Actor>: Send + Sync {
    #[allow(unused)]
    fn join(&mut self) -> JoinFuture<A>;
}

impl<A, F> Joiner<A> for F
where
    A: Actor,
    F: FnMut() -> JoinFuture<A>,
    F: Send + Sync,
{
    fn join(&mut self) -> JoinFuture<A> {
        self()
    }
}

pub trait Spawner<A: Actor> {
    fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static;
}

pub trait Spawnable<S: Spawner<Self>>: Actor {
    fn spawn(self) -> crate::error::Result<(Addr<Self>, DynJoiner<Self>)> {
        let (event_loop, addr) = crate::Environment::unbounded().launch(self);
        // let joiner = S::spawn(futures::FutureExt::map(event_loop, |fut| fut.unwrap()));
        let joiner = S::spawn(event_loop);
        Ok((addr, joiner))
    }
}

pub trait DefaultSpawnable<S: Spawner<Self>>: Actor + Default {
    fn spawn_default() -> crate::error::Result<(Addr<Self>, DynJoiner<Self>)> {
        let (event_loop, addr) = crate::Environment::unbounded().launch(Self::default());
        let joiner = S::spawn(event_loop);
        Ok((addr, joiner))
    }
}

#[cfg(feature = "tokio")]
#[derive(Copy, Clone, Debug, Default)]
pub struct TokioSpawner;
impl<A: Actor> Spawner<A> for TokioSpawner {
    fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static,
    {
        let handle = Arc::new(async_lock::Mutex::new(Some(tokio::spawn(future))));
        Box::new(move || -> JoinFuture<A> {
            let handle = Arc::clone(&handle);
            Box::pin(async move {
                let mut handle: Option<tokio::task::JoinHandle<DynResult<A>>> =
                    handle.lock().await.take();

                if let Some(handle) = handle.take() {
                    // TODO: don't eat the error
                    handle.await.ok().and_then(Result::ok)
                } else {
                    None
                }
            })
        })
    }
}

#[cfg(feature = "tokio")]
impl<A> Spawnable<TokioSpawner> for A where A: Actor {}

#[cfg(feature = "tokio")]
impl<A> DefaultSpawnable<TokioSpawner> for A where A: Actor + Default {}

#[cfg(feature = "async-std")]
#[derive(Copy, Clone, Debug, Default)]
pub struct AsyncStdSpawner;
impl<A: Actor> Spawner<A> for AsyncStdSpawner {
    fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
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
                    // TODO: don 't eat the error
                    handle.await.ok()
                } else {
                    None
                }
            })
        })
    }
}
#[cfg(feature = "async-std")]
impl<A> Spawnable<AsyncStdSpawner> for A where A: Actor {}
#[cfg(feature = "async-std")]
impl<A> DefaultSpawnable<AsyncStdSpawner> for A where A: Actor + Default {}
