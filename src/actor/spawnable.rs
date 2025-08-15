//! Abstractions for spawning and managing actors in an asynchronous environment.
//! Currently hannibal supports both `tokio` and `smol` `async-global-executore`.

use std::any::type_name;

use crate::{Addr, StreamHandler, addr::OwningAddr, environment::Environment};

use super::{Actor, ActorHandle};

/// Enables an actor to spawn itself.
pub trait Spawnable: Actor {
    /// Spawns the actor and returns an `Addr` to it.
    fn spawn(self) -> Addr<Self> {
        self.spawn_owning().detach()
    }

    /// Spawns the actor and returns an [`OwningAddr`] to it.
    fn spawn_owning(self) -> OwningAddr<Self> {
        let environment = Environment::unbounded();
        Self::spawn_owning_in(self, environment)
    }

    /// Spawns an actor in a specific environment and returns an [`OwningAddr`] to it.
    #[doc(hidden)]
    fn spawn_owning_in(self, environment: Environment<Self>) -> OwningAddr<Self> {
        log::trace!("spawn {}", type_name::<Self>());
        let (event_loop, addr) = environment.create_loop(self);
        let handle = ActorHandle::spawn(event_loop);
        OwningAddr::new(addr, handle)
    }
}

/// An actor that can handle a stream of messages.
pub trait StreamSpawnable<T>: Actor + StreamHandler<T::Item>
where
    T: futures::Stream + Unpin + Send + 'static,
    T::Item: 'static + Send,
    Self: StreamHandler<T::Item>,
{
    /// Spawns the actor on the provided stream and returns an `Addr` handle.
    fn spawn_on_stream(self, stream: T) -> crate::error::Result<Addr<Self>> {
        Ok(self.spawn_owning_on_stream(stream)?.detach())
    }

    /// Spawns the actor on the provided stream and returns an `OwningAddr` handle.
    fn spawn_owning_on_stream(self, stream: T) -> crate::error::Result<OwningAddr<Self>> {
        log::trace!("spawn on stream {}", type_name::<Self>());
        let (event_loop, addr) = Environment::unbounded().create_loop_on_stream(self, stream);
        let handle = ActorHandle::spawn(event_loop);
        Ok(OwningAddr::new(addr, handle))
    }
}

/// An Actor that implements [`Default`].
pub trait DefaultSpawnable: Actor + Default {
    /// Spawns a new actor with default configuration.
    fn spawn_default() -> crate::error::Result<Addr<Self>> {
        Ok(Self::spawn_default_owning()?.detach())
    }

    /// Spawns a new actor with default configuration.
    fn spawn_default_owning() -> crate::error::Result<OwningAddr<Self>> {
        log::trace!("spawn defauwning {}", type_name::<Self>());
        let (event_loop, addr) = Environment::unbounded().create_loop(Self::default());
        let handle = ActorHandle::spawn(event_loop);
        Ok(OwningAddr::new(addr, handle))
    }
}

/// Every actor that can be spawned.
impl<A: Actor> Spawnable for A {}

/// Every actor that implements `Default`.
impl<A: Actor + Default> DefaultSpawnable for A {}

/// Actors that implement `StreamHandler`.
impl<A, T> StreamSpawnable<T> for A
where
    A: Actor + StreamHandler<T::Item>,
    T: futures::Stream + Unpin + Send + 'static,
    T::Item: 'static + Send,
{
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    mod spawned_with_tokio {
        use crate::{
            actor::tests::{Ping, TokioActor},
            spawnable::{DefaultSpawnable, Spawnable},
        };

        #[tokio::test]
        async fn spawn() {
            let tokio_actor = TokioActor::<()>::default();
            let mut addr = tokio_actor.spawn();
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }

        #[tokio::test]
        async fn spawn_default() {
            let mut addr = TokioActor::<()>::spawn_default().unwrap();
            assert!(!addr.stopped());

            addr.call(Ping).await.unwrap();
            addr.stop().unwrap();
            addr.await.unwrap()
        }
    }
}
