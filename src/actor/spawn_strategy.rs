use std::time::Duration;
#[cfg_attr(
    not(any(feature = "tokio", feature = "async-std")),
    allow(unused_imports)
)]
use std::{future::Future, pin::Pin, sync::Arc};

use crate::{environment::Environment, Addr, StreamHandler};

#[cfg_attr(
    not(any(feature = "tokio", feature = "async-std")),
    allow(unused_imports)
)]
use super::{Actor, DynResult};

pub type JoinFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

pub trait ActorHandle<A: Actor>: Send + Sync {
    fn join(&mut self) -> JoinFuture<A>;
}

impl<A, F> ActorHandle<A> for F
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
    fn spawn_actor<F>(future: F) -> Box<dyn ActorHandle<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static;

    fn spawn_future<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static;

    fn sleep(duration: Duration) -> impl Future<Output = ()> + Send;
}

pub trait SpawnableWith: Actor {
    fn spawn_with<S: Spawner<Self>>(
        self,
    ) -> crate::error::Result<(Addr<Self>, Box<dyn ActorHandle<Self>>)> {
        let (event_loop, addr) = Environment::unbounded().create_loop(self);
        let handle = S::spawn_actor(event_loop);
        Ok((addr, handle))
    }

    fn spawn_with_in<S: Spawner<Self>>(
        self,
        environment: Environment<Self>,
    ) -> crate::error::Result<(Addr<Self>, Box<dyn ActorHandle<Self>>)> {
        let (event_loop, addr) = environment.create_loop(self);
        let handle = S::spawn_actor(event_loop);
        Ok((addr, handle))
    }
}

impl<A: Actor> SpawnableWith for A {}

pub trait Spawnable<S: Spawner<Self>>: Actor {
    fn spawn(self) -> Addr<Self> {
        self.spawn_and_get_handle().0
    }

    fn spawn_in(self, environment: Environment<Self>) -> Addr<Self> {
        self.spawn_in_and_get_handle(environment).0
    }

    fn spawn_and_get_handle(self) -> (Addr<Self>, Box<dyn ActorHandle<Self>>) {
        self.spawn_in_and_get_handle(Environment::unbounded())
    }

    fn spawn_in_and_get_handle(
        self,
        environment: Environment<Self>,
    ) -> (Addr<Self>, Box<dyn ActorHandle<Self>>) {
        let (event_loop, addr) = environment.create_loop(self);
        let handle = S::spawn_actor(event_loop);
        (addr, handle)
    }
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub(crate) trait SpawnSelf<S: Spawner<Self>>: Actor {
    fn spawn_future<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        S::spawn_future(future);
    }

    fn sleep(duration: Duration) -> impl Future<Output = ()> + Send {
        S::sleep(duration)
    }
}

pub trait StreamSpawnable<S: Spawner<Self>, T>: Actor + StreamHandler<T::Item>
where
    T: futures::Stream + Unpin + Send + 'static,
    T::Item: 'static + Send,
    Self: StreamHandler<T::Item>,
{
    fn spawn_on_stream(self, stream: T) -> crate::error::Result<Addr<Self>> {
        Ok(self.spawn_on_stream_and_get_handle(stream)?.0)
    }

    fn spawn_on_stream_and_get_handle(
        self,
        stream: T,
    ) -> crate::error::Result<(Addr<Self>, Box<dyn ActorHandle<Self>>)> {
        let (event_loop, addr) = Environment::unbounded().create_loop_on_stream(self, stream);
        let handle = S::spawn_actor(event_loop);
        println!("spawned");
        Ok((addr, handle))
    }
}

pub trait DefaultSpawnable<S: Spawner<Self>>: Actor + Default {
    fn spawn_default() -> crate::error::Result<Addr<Self>> {
        Ok(Self::spawn_default_and_get_handle()?.0)
    }

    fn spawn_default_and_get_handle(
    ) -> crate::error::Result<(Addr<Self>, Box<dyn ActorHandle<Self>>)> {
        let (event_loop, addr) = Environment::unbounded().create_loop(Self::default());
        let handle = S::spawn_actor(event_loop);
        Ok((addr, handle))
    }
}

#[cfg(feature = "tokio")]
mod tokio_spawner {
    use super::*;
    #[derive(Copy, Clone, Debug, Default)]
    pub struct TokioSpawner;
    impl<A: Actor> Spawner<A> for TokioSpawner {
        fn spawn_actor<F>(future: F) -> Box<dyn ActorHandle<A>>
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

        fn spawn_future<F>(future: F)
        where
            F: Future<Output = ()> + Send + 'static,
        {
            tokio::spawn(future);
        }

        async fn sleep(duration: Duration) {
            tokio::time::sleep(duration).await;
        }
    }
}
#[cfg(feature = "tokio")]
pub use tokio_spawner::TokioSpawner;

// and now for async-std

#[cfg(feature = "async-std")]
mod async_spawner {
    use super::*;
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
                        // TODO: don 't eat the error
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
}
#[cfg(feature = "async-std")]
pub use async_spawner::AsyncStdSpawner;

#[macro_export]
macro_rules! impl_spawn_traits {
    ($spawner_type:ty) => {
        impl<A> Spawnable<$spawner_type> for A where A: Actor {}
        impl<A> SpawnSelf<$spawner_type> for A where A: Actor {}

        impl<A, T> StreamSpawnable<$spawner_type, T> for A
        where
            A: Actor + StreamHandler<T::Item>,
            T: futures::Stream + Unpin + Send + 'static,
            T::Item: 'static + Send,
        {
        }

        impl<A> DefaultSpawnable<$spawner_type> for A where A: Actor + Default {}
    };
}

cfg_if::cfg_if! {
    if #[cfg(all(feature = "tokio", not(feature = "async-std")))] {
        impl_spawn_traits!(TokioSpawner);
        pub type DefaultSpawner = TokioSpawner;
    } else if #[cfg( all(not(feature = "tokio"), feature = "async-std") )] {
        impl_spawn_traits!(AsyncStdSpawner);
        pub type DefaultSpawner = AsyncStdSpawner;
    } else if #[cfg(all(feature = "tokio", feature = "async-std") )] {
        // if both are enabled, we can not provice a default spawner
    } else {
        // if both are disabled, we can not provice a default spawner either
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #[cfg(feature = "tokio")]
    mod spawned_with_tokio {
        use crate::{
            actor::tests::{spawned_with_tokio::TokioActor, Ping},
            spawn_strategy::{DefaultSpawnable, Spawnable, TokioSpawner},
        };

        #[tokio::test]
        async fn spawn() {
            let tokio_actor = TokioActor::default();
            let mut addr = <TokioActor<()> as Spawnable<TokioSpawner>>::spawn(tokio_actor);
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }

        #[tokio::test]
        async fn spawn_default() {
            let mut addr =
                <TokioActor<()> as DefaultSpawnable<TokioSpawner>>::spawn_default().unwrap();
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }
    }

    #[cfg(feature = "async-std")]
    mod spawned_with_asyncstd {
        use crate::{
            actor::tests::{spawned_with_asyncstd::AsyncStdActor, Ping},
            spawn_strategy::{AsyncStdSpawner, DefaultSpawnable, Spawnable},
        };

        #[async_std::test]
        async fn spawn() {
            let tokio_actor = AsyncStdActor::default();
            let mut addr = <AsyncStdActor<()> as Spawnable<AsyncStdSpawner>>::spawn(tokio_actor);
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }

        #[async_std::test]
        async fn spawn_default() {
            let mut addr =
                <AsyncStdActor<()> as DefaultSpawnable<AsyncStdSpawner>>::spawn_default().unwrap();
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }
    }
}
