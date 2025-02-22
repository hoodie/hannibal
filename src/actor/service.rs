//! Provides the `Service` trait and implementations.
//!
//! A service is a special type of actor that can be registered, replaced, or unregistered
//! in a global registry. Services are typically used for actors that need to be accessed
//! globally and do not require ownership by other actors.
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::LazyLock,
};

use futures::FutureExt as _;

use super::{spawner::Spawner, *};

use crate::{Addr, environment::Environment};

type AnyBox = Box<dyn Any + Send + Sync>;

static REGISTRY: LazyLock<async_lock::RwLock<HashMap<TypeId, AnyBox>>> =
    LazyLock::new(Default::default);

/// Service Related
///
/// An actor that implements the [`Service`] trait can be registered, unregistered and replaced via an `Addr` as a service.
#[cfg(any(feature = "tokio", feature = "async-std"))]
impl<A: Service> Addr<A> {
    /// Register an actor as a service.
    ///
    /// If there already is a running service, this method will return an error.
    /// Use [`Addr::replace()`] to replace a running service.
    pub async fn register(self) -> crate::error::Result<(Self, Option<Self>)> {
        let key = TypeId::of::<A>();
        let mut registry = REGISTRY.write().await;

        let replaced = if let Some(existing) = registry.get(&key) {
            if existing
                .downcast_ref::<Addr<A>>()
                .is_some_and(Addr::stopped)
            {
                registry
                    .insert(key, Box::new(self.clone()))
                    .and_then(|addr| addr.downcast::<Addr<A>>().ok())
                    .map(|addr| *addr)
            } else {
                return Err(crate::error::ActorError::ServiceStillRunning);
            }
        } else {
            registry
                .insert(key, Box::new(self.clone()))
                .and_then(|addr| addr.downcast::<Addr<A>>().ok())
                .map(|addr| *addr)
        };

        Ok((self, replaced))
    }

    /// Replace a running service.
    ///
    /// The old service is returned, it is not stopped until you stop or drop it.
    pub async fn replace(self) -> Option<Self> {
        let key = TypeId::of::<A>();
        let mut registry = REGISTRY.write().await;
        registry
            .insert(key, Box::new(self.clone()))
            .and_then(|addr| addr.downcast::<Addr<A>>().ok())
            .map(|addr| *addr)
    }

    /// Unregister a service.
    pub async fn unregister() -> Option<Addr<A>> {
        let key = TypeId::of::<A>();
        let mut registry = REGISTRY.write().await;
        registry
            .remove(&key)
            .and_then(|addr| addr.downcast::<Addr<A>>().ok())
            .map(|addr| *addr)
    }
}

/// A service is an actor that does not need to be owned
///
/// Some functionality of the service is available on the [`Addr`](`Addr`#impl-Addr%3CA%3E) of the service.
#[cfg(any(feature = "tokio", feature = "async-std"))]
pub trait Service: Actor + Default {
    /// Setup the service.
    ///
    /// Usually the service is spawned when the first actor accesses it.
    /// If you want to ensure that the service is running before any actor
    /// accesses it, you can call this method.
    fn setup() -> impl Future<Output = DynResult<()>> {
        Self::from_registry_and_spawn().map(|_| Ok(()))
    }

    /// Check if the service is already running.
    fn already_running() -> impl Future<Output = Option<bool>> {
        async {
            let key = TypeId::of::<Self>();
            let registry = REGISTRY.read().await;
            registry
                .get(&key)
                .and_then(|addr| addr.downcast_ref::<Addr<Self>>().map(Addr::stopped))
        }
    }

    /// Get the service from the registry.
    fn from_registry() -> impl Future<Output = Addr<Self>> {
        Self::from_registry_and_spawn()
    }

    /// Get the service from the registry synchronously if it is running.
    fn try_from_registry() -> Option<Addr<Self>> {
        let key = TypeId::of::<Self>();
        REGISTRY
            .try_read()?
            .get(&key)
            .and_then(|addr| addr.downcast_ref::<Addr<Self>>())
            .filter(|addr| addr.running())
            .cloned()
    }
}

#[cfg(not(any(feature = "tokio", feature = "async-std")))]
pub trait Service<S: Spawner<Self>>: Actor + Default {
    fn setup() -> impl Future<Output = ()> {
        Self::from_registry_and_spawn().map(|_| ())
    }

    fn from_registry() -> impl Future<Output = Addr<Self>> {
        Self::from_registry_and_spawn()
    }

    #[allow(clippy::async_yields_async)]
    fn from_registry_and_spawn() -> impl Future<Output = Addr<Self>> {
        async {
            let key = TypeId::of::<Self>();

            let mut registry = REGISTRY.write().await;

            if let Some(addr) = registry
                .get_mut(&key)
                .and_then(|addr| addr.downcast_ref::<Addr<Self>>())
                .map(ToOwned::to_owned)
                .filter(Addr::running)
            {
                addr
            } else {
                let (event_loop, addr) = Environment::unbounded().create_loop(Self::default());
                S::spawn_actor(event_loop);
                registry.insert(key, Box::new(addr.clone()));
                addr
            }
        }
    }
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub(crate) trait SpawnableService<S: Spawner<Self>>: Service {
    #[allow(clippy::async_yields_async)]
    fn from_registry_and_spawn() -> impl Future<Output = Addr<Self>> {
        async {
            let key = TypeId::of::<Self>();

            let mut registry = REGISTRY.write().await; // this is the only reason for the async block

            if let Some(addr) = registry
                .get_mut(&key)
                .and_then(|addr| addr.downcast_ref::<Addr<Self>>())
                .map(ToOwned::to_owned)
                .filter(Addr::running)
            {
                addr
            } else {
                let (event_loop, addr) = Environment::unbounded().create_loop(Self::default());
                S::spawn_actor(event_loop);
                registry.insert(key, Box::new(addr.clone()));
                addr
            }
        }
    }
}

#[cfg(any(
    all(feature = "tokio", not(feature = "async-std")),
    all(not(feature = "tokio"), feature = "async-std")
))]
impl<A, S> SpawnableService<S> for A
where
    A: Service,
    A: spawner::Spawnable<S>,
    S: Spawner<A>,
{
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    #[cfg(feature = "tokio")]
    mod spawned_with_tokio {
        use crate::{
            Service,
            actor::tests::{Identify, Ping, spawned_with_tokio::TokioActor},
            prelude::Spawnable as _,
            spawner::{SpawnableWith, TokioSpawner},
        };

        #[test_log::test(tokio::test)]
        async fn register_as_service() {
            type Svc = TokioActor<u32>;
            let (addr, mut handle) = Svc::new(1337).spawn_with::<TokioSpawner>().unwrap();
            let (mut addr, _) = addr.register().await.unwrap();
            assert_eq!(addr.call(Identify).await.unwrap(), 1337);
            assert_eq!(addr.call(Identify).await.unwrap(), 1337);
            addr.stop().unwrap();
            handle.join().await.unwrap();
        }

        #[test_log::test(tokio::test)]
        async fn get_service_from_registry() {
            type Svc = TokioActor<u64>;
            let mut svc_addr = Svc::from_registry().await;
            assert!(!svc_addr.stopped());

            svc_addr.call(Ping).await.unwrap();

            svc_addr.stop().unwrap();
            svc_addr.await.unwrap();
        }

        #[test_log::test(tokio::test)]
        async fn reregistering_service_only_if_stopped() {
            // Define the service type as TokioActor with u64
            type Svc = TokioActor<((), ())>;

            // Spawn a new service instance with TokioSpawner and unwrap the result
            let (first_svc, replaced) = crate::build(Svc::new(1337))
                .unbounded()
                .spawn()
                .register()
                .await
                .unwrap();

            assert!(replaced.is_none());
            assert_eq!(first_svc.call(Identify).await, Ok(1337));

            // stop the service
            let mut first_svc_again = Svc::from_registry().await;
            assert_eq!(first_svc_again.call(Identify).await, Ok(1337));

            first_svc_again.stop().unwrap();
            assert!(!first_svc_again.stopped());
            first_svc_again.await.unwrap();

            // register a new service instance
            let (second_svc, replaced_first) = Svc::new(1338).spawn().register().await.unwrap();
            assert!(replaced_first.is_some());
            assert_eq!(second_svc.call(Identify).await, Ok(1338));
            assert!(replaced_first.unwrap().call(Identify).await.is_err());

            // registering without stopping the service should return None
            assert!(Svc::new(1338).spawn().register().await.is_err());
        }
    }

    #[cfg(feature = "async-std")]
    mod spawned_with_asyncstd {
        use crate::{
            Service,
            actor::tests::{Identify, Ping, spawned_with_asyncstd::AsyncStdActor},
            spawner::{AsyncStdSpawner, SpawnableWith},
        };

        #[async_std::test]
        async fn register_as_service() {
            type Svc = AsyncStdActor<u32>;
            let (addr, mut handle) = Svc::new(1337).spawn_with::<AsyncStdSpawner>().unwrap();
            addr.register().await.unwrap();
            let mut svc_addr = Svc::from_registry().await;
            assert_eq!(svc_addr.call(Identify).await.unwrap(), 1337);
            assert_eq!(svc_addr.call(Identify).await.unwrap(), 1337);

            svc_addr.stop().unwrap();
            handle.join().await.unwrap();
        }

        #[test_log::test(tokio::test)]
        async fn get_service_from_registry() {
            type Svc = AsyncStdActor<u64>;
            Svc::setup().await.unwrap();
            let mut svc_addr = Svc::from_registry().await;
            assert!(!svc_addr.stopped());

            svc_addr.call(Ping).await.unwrap();

            svc_addr.stop().unwrap();
            svc_addr.await.unwrap();
        }

        #[async_std::test]
        async fn get_service_from_registry_without_set() {
            type Svc = AsyncStdActor<f64>;
            let mut svc_addr = Svc::from_registry().await;
            assert!(!svc_addr.stopped());

            svc_addr.call(Ping).await.unwrap();
            svc_addr.stop().unwrap();
            svc_addr.await.unwrap();
        }
    }
}
