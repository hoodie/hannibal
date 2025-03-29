use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use crate::{Addr, StreamHandler, environment::Environment};

use super::{Actor, DynResult};

#[cfg(feature = "async_runtime")]
mod async_spawner;

#[cfg(feature = "tokio_runtime")]
mod tokio_spawner;

#[cfg(feature = "smol_runtime")]
mod smol_spawner;

pub type JoinFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

// TODO: rename to `Handle`
pub(crate) type DynJoiner<A> = Box<dyn Joiner<A>>;
pub trait Joiner<A: Actor>: Send + Sync {
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
    fn spawn_actor<F>(future: F) -> Box<dyn Joiner<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static;

    fn spawn_future<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static;

    fn sleep(duration: Duration) -> impl Future<Output = ()> + Send;
}

pub trait SpawnableWith: Actor {
    fn spawn_with<S: Spawner<Self>>(self) -> crate::error::Result<(Addr<Self>, DynJoiner<Self>)> {
        let (event_loop, addr) = Environment::unbounded().launch(self);
        let joiner = S::spawn_actor(event_loop);
        Ok((addr, joiner))
    }

    fn spawn_with_in<S: Spawner<Self>>(
        self,
        environment: Environment<Self>,
    ) -> crate::error::Result<(Addr<Self>, DynJoiner<Self>)> {
        let (event_loop, addr) = environment.launch(self);
        let joiner = S::spawn_actor(event_loop);
        Ok((addr, joiner))
    }
}

impl<A: Actor> SpawnableWith for A {}

pub trait Spawnable<S: Spawner<Self>>: Actor {
    fn spawn(self) -> Addr<Self> {
        self.spawn_and_get_joiner().0
    }

    fn spawn_in(self, environment: Environment<Self>) -> Addr<Self> {
        self.spawn_in_and_get_joiner(environment).0
    }

    fn spawn_and_get_joiner(self) -> (Addr<Self>, DynJoiner<Self>) {
        self.spawn_in_and_get_joiner(Environment::unbounded())
    }

    fn spawn_in_and_get_joiner(
        self,
        environment: Environment<Self>,
    ) -> (Addr<Self>, DynJoiner<Self>) {
        let (event_loop, addr) = environment.launch(self);
        let joiner = S::spawn_actor(event_loop);
        (addr, joiner)
    }
}

pub(crate) trait SpawnableHack<S: Spawner<Self>>: Actor {
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
        Ok(self.spawn_on_stream_and_get_joiner(stream)?.0)
    }

    fn spawn_on_stream_and_get_joiner(
        self,
        stream: T,
    ) -> crate::error::Result<(Addr<Self>, DynJoiner<Self>)> {
        let (event_loop, addr) = Environment::unbounded().launch_on_stream(self, stream);
        let joiner = S::spawn_actor(event_loop);
        println!("spawned");
        Ok((addr, joiner))
    }
}

pub trait DefaultSpawnable<S: Spawner<Self>>: Actor + Default {
    fn spawn_default() -> crate::error::Result<Addr<Self>> {
        Ok(Self::spawn_default_and_get_joiner()?.0)
    }

    fn spawn_default_and_get_joiner() -> crate::error::Result<(Addr<Self>, DynJoiner<Self>)> {
        let (event_loop, addr) = Environment::unbounded().launch(Self::default());
        let joiner = S::spawn_actor(event_loop);
        Ok((addr, joiner))
    }
}

cfg_if::cfg_if! {
    if #[cfg(all(
        feature = "tokio_runtime",
        not(any(feature = "async_runtime", feature = "smol_runtime"))
    ))] {

        compile_error!("don't use tokio");
        impl<A> Spawnable<TokioSpawner> for A where A: Actor {}
        impl<A> SpawnableHack<TokioSpawner> for A where A: Actor {}

        impl<A, T> StreamSpawnable<TokioSpawner, T> for A
        where
            A: Actor + StreamHandler<T::Item>,
            T: futures::Stream + Unpin + Send + 'static,
            T::Item: 'static + Send,
        {}

        impl<A> DefaultSpawnable<TokioSpawner> for A where A: Actor + Default {}
        pub type DefaultSpawner = TokioSpawner;

    } else if #[cfg( all(
        feature = "async_runtime",
        not(any(feature = "tokio_runtime", feature="smol_runtime")),
    ) )] {
        compile_error!("don't use async-std");

        impl<A> Spawnable<AsyncStdSpawner> for A where A: Actor {}
        impl<A> SpawnableHack<AsyncStdSpawner> for A where A: Actor {}

        impl<A, T> StreamSpawnable<AsyncStdSpawner, T> for A
        where
            A: Actor + StreamHandler<T::Item>,
            T: futures::Stream + Unpin + Send + 'static,
            T::Item: 'static + Send,
        {
        }

        impl<A> DefaultSpawnable<AsyncStdSpawner> for A where A: Actor + Default {}
        pub type DefaultSpawner = AsyncStdSpawner;


    } else if #[cfg( all(
        feature = "smol_runtime",
        not(any(feature="tokio_runtime", feature="async_runtime")),
    ) )] {
        use smol_spawner::SmolSpawner;
        compile_error!("USE SMOL YES");

        impl<A> Spawnable<SmolSpawner> for A where A: Actor {}
        impl<A> SpawnableHack<SmolSpawner> for A where A: Actor {}

        impl<A, T> StreamSpawnable<SmolSpawner, T> for A
        where
            A: Actor + StreamHandler<T::Item>,
            T: futures::Stream + Unpin + Send + 'static,
            T::Item: 'static + Send,
        {
        }

        impl<A> DefaultSpawnable<SmolSpawner> for A where A: Actor + Default {}
        pub type DefaultSpawner = SmolSpawner;

    } else if #[cfg(all(feature = "tokio_runtime", feature = "async_runtime", feature = "smol") )] {
        compile_error!("all runtimes are on");
        // if both are enabled, we can not provice a default spawner
    } else {
        compile_error!("no runtimes are on");
        // if both are disabled, we can not provice a default spawner either
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "tokio_runtime")]
    mod spawned_with_tokio {
        use crate::{
            actor::tests::{Ping, spawned_with_tokio::TokioActor},
            spawn_strategy::{DefaultSpawnable, Spawnable, TokioSpawner},
        };

        #[test_log::test(tokio::test)]
        async fn spawn() {
            let tokio_actor = TokioActor::default();
            let mut addr = <TokioActor<()> as Spawnable<TokioSpawner>>::spawn(tokio_actor);
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }

        #[test_log::test(tokio::test)]
        async fn spawn_default() {
            let mut addr =
                <TokioActor<()> as DefaultSpawnable<TokioSpawner>>::spawn_default().unwrap();
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }
    }

    #[cfg(feature = "async_runtime")]
    mod spawned_with_asyncstd {
        use crate::{
            actor::tests::{Ping, spawned_with_asyncstd::AsyncStdActor},
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
