//! Abstractions for spawning and managing actors in an asynchronous environment.
//! Currently hannibal supports both `tokio` and `async-std`. Custom spawners can be implemented.

use std::future::Future;
use std::sync::Arc;

use crate::{Addr, DynResult, StreamHandler, addr::OwningAddr, environment::Environment};

#[cfg_attr(not(feature = "runtime"), allow(unused_imports))]
use super::Actor;

mod actor_handle;

pub use actor_handle::{ActorHandle, JoinFuture};

pub fn spawn_actor<A, F>(event_loop: F) -> ActorHandle<A>
where
    A: Actor,
    F: Future<Output = DynResult<A>> + Send + 'static,
{
    let task = async_global_executor::spawn(event_loop);
    let handle = Arc::new(async_lock::Mutex::new(Some(task)));
    let detach_handle = Arc::clone(&handle);

    ActorHandle::new(move || -> JoinFuture<A> {
        let handle = Arc::clone(&handle);
        Box::pin(async move {
            let handle_opt = handle.lock().await.take();

            if let Some(handle) = handle_opt {
                handle.await.ok()
            } else {
                None
            }
        })
    })
    .with_detach_fn(move || {
        let mut handle = detach_handle.lock_blocking().take();
        if let Some(handle) = handle.take() {
            handle.detach();
        }
    })
}

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
        let (event_loop, addr) = environment.create_loop(self);
        let handle = spawn_actor(event_loop);
        OwningAddr::new(addr, handle)
    }
}
impl<A: Actor> Spawnable for A {}

/// An actor that can handle a stream of messages.
pub trait StreamSpawnable<T>: Actor + StreamHandler<T::Item>
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
        let handle = spawn_actor(event_loop);
        Ok(OwningAddr::new(addr, handle))
    }
}

impl<A, T> StreamSpawnable<T> for A
where
    A: Actor + StreamHandler<T::Item>,
    T: futures::Stream + Unpin + Send + 'static,
    T::Item: 'static + Send,
{
}

pub trait DefaultSpawnable: Actor + Default {
    fn spawn_default() -> crate::error::Result<Addr<Self>> {
        Ok(Self::spawn_default_owning()?.detach())
    }

    fn spawn_default_owning() -> crate::error::Result<OwningAddr<Self>> {
        let (event_loop, addr) = Environment::unbounded().create_loop(Self::default());
        let handle = spawn_actor(event_loop);
        Ok(OwningAddr::new(addr, handle))
    }
}

impl<A: Actor + Default> DefaultSpawnable for A {}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #[cfg(feature = "runtime")]
    mod spawned_with_tokio {
        use crate::{
            actor::tests::{Ping, spawned_with_tokio::TokioActor},
            spawner::{DefaultSpawnable, Spawnable},
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
