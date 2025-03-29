//! Abstractions for spawning and managing actors in an asynchronous environment.
//! Currently hannibal supports both `tokio` and `async-std`. Custom spawners can be implemented.

use std::time::Duration;
#[cfg_attr(not(feature = "runtime"), allow(unused_imports))]
use std::{future::Future, pin::Pin};

use crate::{Addr, DynResult, StreamHandler, addr::OwningAddr, environment::Environment};

#[cfg_attr(not(feature = "runtime"), allow(unused_imports))]
use super::Actor;

#[cfg(feature = "tokio_runtime")]
mod tokio_spawner;
#[cfg(feature = "tokio_runtime")]
pub use tokio_spawner::TokioSpawner;

#[cfg(feature = "async_runtime")]
mod async_spawner;
#[cfg(feature = "async_runtime")]
pub use async_spawner::AsyncStdSpawner;

#[cfg(feature = "smol_runtime")]
mod smol_spawner;
#[cfg(feature = "smol_runtime")]
pub use smol_spawner::SmolSpawner;

/// A future that resolves to an actor.
pub type JoinFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

pub struct ActorHandle<A> {
    join_fn: Box<dyn FnMut() -> JoinFuture<A>>,
    detach_fn: Option<Box<dyn FnOnce()>>,
}

impl<A> ActorHandle<A> {
    pub fn new<F>(join_fn: F) -> Self
    where
        F: FnMut() -> JoinFuture<A> + 'static,
    {
        Self {
            join_fn: Box::new(join_fn),
            detach_fn: None,
        }
    }

    pub fn with_detach_fn<F: FnOnce() + 'static>(mut self, detach_fn: F) -> Self {
        self.detach_fn = Some(Box::new(detach_fn));
        self
    }

    pub fn join(&mut self) -> JoinFuture<A> {
        (self.join_fn)()
    }

    pub fn detach(self) {
        if let Some(detach_fn) = self.detach_fn {
            detach_fn();
        }
    }
}

/// Encapsulates spawning actors and futures, as well as sleeping.
///
/// You should implement at this trait if you want to build a custom spawner.
pub trait Spawner<A: Actor> {
    fn spawn_actor<F: Future<Output = DynResult<A>> + Send + 'static>(future: F) -> ActorHandle<A>;
    fn spawn_future<F: Future<Output = ()> + Send + 'static>(future: F);
    fn sleep(duration: Duration) -> impl Future<Output = ()> + Send;
}

/// Enables an actor to spawn itself with a specific [`Spawner`].
///
/// See the [`custom_spawner`](../example/custom_spawner.rs) example for a demonstration.
pub trait SpawnableWith: Actor {
    fn spawn_with<S: Spawner<Self>>(self) -> (Addr<Self>, ActorHandle<Self>) {
        let (event_loop, addr) = Environment::unbounded().create_loop(self);
        let handle = S::spawn_actor(event_loop);
        (addr, handle)
    }

    fn spawn_with_in<S: Spawner<Self>>(
        self,
        environment: Environment<Self>,
    ) -> (Addr<Self>, ActorHandle<Self>) {
        let (event_loop, addr) = environment.create_loop(self);
        let handle = S::spawn_actor(event_loop);
        (addr, handle)
    }
}

impl<A: Actor> SpawnableWith for A {}

/// Enables an actor to spawn itself.
pub trait Spawnable<S: Spawner<Self>>: Actor {
    /// Spawns the actor and returns an `Addr` to it.
    fn spawn(self) -> Addr<Self> {
        self.spawn_owning().detach()
    }

    /// Spawns the actor and returns an [`OwningAddr`] to it.
    fn spawn_owning(self) -> OwningAddr<Self> {
        let environment = Environment::unbounded();
        let (event_loop, addr) = environment.create_loop(self);
        log::trace!(
            "spawning actor with custom environment {}",
            std::any::type_name::<S>()
        );
        let handle = S::spawn_actor(event_loop);
        OwningAddr::new(addr, handle)
    }
    // }

    // pub(crate) trait SpawnableIn<S: Spawner<Self>>: Actor {
    /// Spawns an actor in a specific environment and returns an [`OwningAddr`] to it.
    #[doc(hidden)]
    fn spawn_owning_in(self, environment: Environment<Self>) -> OwningAddr<Self> {
        let (event_loop, addr) = environment.create_loop(self);
        let handle = S::spawn_actor(event_loop);
        OwningAddr::new(addr, handle)
    }
}

#[cfg(feature = "runtime")]
/// Enables an actor to spawn futures and sleep.
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

/// An actor that can handle a stream of messages.
pub trait StreamSpawnable<S: Spawner<Self>, T>: Actor + StreamHandler<T::Item>
where
    T: futures::Stream + Unpin + Send + 'static,
    T::Item: 'static + Send,
    Self: StreamHandler<T::Item>,
{
    fn spawn_on_stream(self, stream: T) -> crate::error::Result<Addr<Self>> {
        Ok(self.spawn_owning_on_stream(stream)?.detach())
    }

    fn spawn_owning_on_stream(self, stream: T) -> crate::error::Result<OwningAddr<Self>> {
        let (event_loop, addr) = Environment::unbounded().create_loop_on_stream(self, stream);
        let handle = S::spawn_actor(event_loop);
        Ok(OwningAddr::new(addr, handle))
    }
}

pub trait DefaultSpawnable<S: Spawner<Self>>: Actor + Default {
    fn spawn_default() -> crate::error::Result<Addr<Self>> {
        Ok(Self::spawn_owning()?.detach())
    }

    fn spawn_owning() -> crate::error::Result<OwningAddr<Self>> {
        let (event_loop, addr) = Environment::unbounded().create_loop(Self::default());
        let handle = S::spawn_actor(event_loop);
        Ok(OwningAddr::new(addr, handle))
    }
}

#[macro_export]
#[doc(hidden)]
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
    if #[cfg(
        all(feature = "tokio_runtime",
        all(not(feature = "async_runtime"),not(feature = "smol_runtime")))
    )] {
        impl_spawn_traits!(TokioSpawner);
        pub type DefaultSpawner = TokioSpawner;
    } else if #[cfg(
        all(
            feature = "async_runtime",
            all(not(feature = "tokio_runtime"),not(feature = "smol_runtime"))
        )
    )] {
        impl_spawn_traits!(AsyncStdSpawner);
        pub type DefaultSpawner = AsyncStdSpawner;
    } else if #[cfg(
        all(
            feature = "smol_runtime",
            all(not(feature = "tokio_runtime"),not(feature = "async_runtime"))
        )
    )] {
        impl_spawn_traits!(SmolSpawner);
        pub type DefaultSpawner = SmolSpawner;
    } else {
        // if both are disabled, we can not provide a default spawner either
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #[cfg(feature = "tokio_runtime")]
    mod spawned_with_tokio {
        use crate::{
            actor::tests::{Ping, spawned_with_tokio::TokioActor},
            spawner::{DefaultSpawnable, Spawnable, TokioSpawner},
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

    #[cfg(feature = "async_runtime")]
    mod spawned_with_asyncstd {
        use crate::{
            actor::tests::{Ping, spawned_with_asyncstd::AsyncStdActor},
            spawner::{AsyncStdSpawner, DefaultSpawnable, Spawnable},
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

    #[cfg(feature = "smol_runtime")]
    mod spawned_with_smol {
        use crate::{
            actor::tests::{Ping, spawned_with_smol::SmolActor},
            spawner::{DefaultSpawnable, SmolSpawner, Spawnable},
        };
        use macro_rules_attribute::apply;
        use smol_macros::test;

        #[apply(test!)]
        async fn spawn() {
            let tokio_actor = SmolActor::default();
            let mut addr = <SmolActor<()> as Spawnable<SmolSpawner>>::spawn(tokio_actor);
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }

        #[apply(test)]
        async fn spawn_default() {
            let mut addr =
                <SmolActor<()> as DefaultSpawnable<SmolSpawner>>::spawn_default().unwrap();
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }
    }
}
