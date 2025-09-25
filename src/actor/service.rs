//! Provides the `Service` trait and implementations.
//!
//! A service is a special type of actor that can be registered, replaced, or unregistered
//! in a global registry. Services are typically used for actors that need to be accessed
//! globally and do not require ownership by other actors.
//!
//! ```
//! # use hannibal::prelude::*;
//! # use std::collections::HashMap;
//! #[derive(Debug, Default, Actor, Service)]
//! struct StorageService {
//!     storage: HashMap<String, String>,
//! }
//!
//! #[message]
//! struct Store(&'static str, &'static str);
//!
//! #[message(response = Option<String>)]
//! struct Retrieve(&'static str);
//!
//! impl Handler<Store> for StorageService {
//!     async fn handle(&mut self, _: &mut Context<Self>, Store(key, value): Store) {
//!         self.storage.insert(key.to_string(), value.to_string());
//!     }
//! }
//!
//! impl Handler<Retrieve> for StorageService {
//!     async fn handle(&mut self, _: &mut Context<Self>, Retrieve(key): Retrieve) -> Option<String> {
//!         self.storage.get(key).cloned()
//!     }
//! }
//!
//! # #[hannibal::main]
//! # async fn main() {
//! StorageService::from_registry().await
//!     .send(Store("password", "hello world")).await
//!     .unwrap();
//!
//! // now anywhere else get a reference to the same instance of `StorageService`
//! let result = StorageService::from_registry().await
//!     .call(Retrieve("password")).await
//!     .unwrap();
//!
//! println!("retrieved: {result:?}");
//! # }
//! ```
//!
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::LazyLock,
};

use futures::FutureExt as _;

use super::*;

use crate::{Addr, environment::Environment};

type AnyBox = Box<dyn Any + Send + Sync>;

static REGISTRY: LazyLock<async_lock::RwLock<HashMap<TypeId, AnyBox>>> =
    LazyLock::new(Default::default);

/// Service Related
///
/// An actor that implements the [`Service`] trait can be registered, unregistered and replaced via an `Addr` as a service.
impl<A: Service> Addr<A> {
    /// Register an actor as a service.
    ///
    /// If there already is a running service, this method will return an error.
    /// Use [`Addr::replace()`] to replace a running service.
    pub async fn register(self) -> crate::error::Result<(Self, Option<Self>)> {
        let key = TypeId::of::<A>();
        let mut registry = REGISTRY.write().await;

        log::trace!("registering service {}", std::any::type_name::<A>());

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
        log::trace!("replacing service {}", std::any::type_name::<A>());
        let mut registry = REGISTRY.write().await;
        registry
            .insert(key, Box::new(self.clone()))
            .and_then(|addr| addr.downcast::<Addr<A>>().ok())
            .map(|addr| *addr)
    }

    /// Unregister a service.
    pub async fn unregister(self) -> Option<Addr<A>> {
        let key = TypeId::of::<A>();
        log::trace!("unregistering service {}", std::any::type_name::<A>());
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
        log::trace!(
            "getting service from registry {}",
            std::any::type_name::<Self>()
        );
        Self::from_registry_and_spawn()
    }

    /// Get the service from the registry synchronously if it is running.
    fn try_from_registry() -> Option<Addr<Self>> {
        let key = TypeId::of::<Self>();
        log::trace!("trying to get service from registry");
        REGISTRY
            .try_read()?
            .get(&key)
            .and_then(|addr| addr.downcast_ref::<Addr<Self>>())
            .filter(|addr| addr.running())
            .cloned()
    }

    /// Unregister a service.
    fn unregister() -> impl Future<Output = Option<Addr<Self>>> {
        async {
            let key = TypeId::of::<Self>();
            log::trace!("unregistering service {}", std::any::type_name::<Self>());
            let mut registry = REGISTRY.write().await;
            registry
                .remove(&key)
                .and_then(|addr| addr.downcast::<Addr<Self>>().ok())
                .map(|addr| *addr)
        }
    }
}

pub(crate) trait SpawnableService: Service {
    #[allow(clippy::async_yields_async)]
    fn from_registry_and_spawn() -> impl Future<Output = Addr<Self>> {
        log::trace!(
            "spawning new instance of {} service in registry",
            std::any::type_name::<Self>()
        );
        async {
            let key = TypeId::of::<Self>();

            let mut registry = REGISTRY.write().await; // this is the only reason for the async block

            if let Some(addr) = registry
                .get_mut(&key)
                .and_then(|addr| addr.downcast_ref::<Addr<Self>>())
                .map(ToOwned::to_owned)
                .filter(Addr::running)
            {
                log::trace!("service already running {}", std::any::type_name::<Self>());
                addr
            } else {
                log::trace!("spawning new service {}", std::any::type_name::<Self>());
                let (event_loop, addr) = Environment::unbounded().create_loop(Self::default());
                let handle = ActorHandle::spawn(event_loop);
                handle.detach();
                registry.insert(key, Box::new(addr.clone()));
                debug_assert!(addr.ping().await.is_ok(), "service failed ping");
                addr
            }
        }
    }
}

impl<A> SpawnableService for A where A: Service {}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::{
        Service,
        actor::tests::{Identify, Ping, TokioActor},
        prelude::Spawnable as _,
    };

    #[test_log::test(crate::test)]
    async fn get_service_from_registry() {
        #[derive(Default)]
        struct Me;
        type Svc = TokioActor<Me>;
        let mut svc_addr = Svc::from_registry().await;
        assert!(!svc_addr.stopped());

        svc_addr.call(Ping).await.unwrap();

        svc_addr.stop().unwrap();
        svc_addr.await.unwrap();
    }

    #[test_log::test(crate::test)]
    async fn reregistering_service_only_if_stopped() {
        #[derive(Default)]
        struct Me;
        type Svc = TokioActor<Me>;

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

    #[test_log::test(crate::test)]
    async fn replace_service() {
        #[derive(Default)]
        struct Me;
        type Svc = TokioActor<Me>;

        // Register initial service
        let (first_svc, _) = Svc::new(1000).spawn().register().await.unwrap();
        assert_eq!(first_svc.call(Identify).await, Ok(1000));

        // Replace with new service
        let second_svc = Svc::new(2000).spawn();
        let replaced = second_svc.replace().await;

        // Verify the replaced service is the old one
        assert!(replaced.is_some());
        let old_svc = replaced.unwrap();
        assert_eq!(old_svc.call(Identify).await, Ok(1000));

        // Verify the new service is now in the registry
        let from_registry = Svc::from_registry().await;
        assert_eq!(from_registry.call(Identify).await, Ok(2000));

        // Clean up
        let mut current_svc = Svc::from_registry().await;
        current_svc.stop().unwrap();
        current_svc.await.unwrap();
    }

    #[test_log::test(crate::test)]
    async fn replace_service_when_none_exists() {
        #[derive(Default)]
        struct Me;
        type Svc = TokioActor<Me>;

        // Replace when no service exists should return None
        let new_svc = Svc::new(3000).spawn();
        let replaced = new_svc.replace().await;
        assert!(replaced.is_none());

        // Verify the new service is now in the registry
        let from_registry = Svc::from_registry().await;
        assert_eq!(from_registry.call(Identify).await, Ok(3000));

        // Clean up
        let mut current_svc = Svc::from_registry().await;
        current_svc.stop().unwrap();
        current_svc.await.unwrap();
    }

    #[test_log::test(crate::test)]
    async fn unregister_service() {
        #[derive(Default)]
        struct Me;
        type Svc = TokioActor<Me>;

        // Register a service
        let (svc, _) = Svc::new(42).spawn().register().await.unwrap();
        assert_eq!(svc.call(Identify).await, Ok(42));

        // Unregister the service
        let unregistered = svc.unregister().await;
        assert!(unregistered.is_some());

        let unregistered2 = Svc::unregister().await;
        assert!(unregistered2.is_none());

        let unregistered_svc = unregistered.unwrap();
        assert_eq!(unregistered_svc.call(Identify).await, Ok(42));

        // Verify the service is no longer in the registry
        // from_registry should spawn a new default instance
        let new_from_registry = Svc::from_registry().await;
        assert_eq!(new_from_registry.call(Identify).await, Ok(0)); // Default value
    }

    #[test_log::test(crate::test)]
    async fn already_running_service() {
        #[derive(Default)]
        struct Me;
        type Svc = TokioActor<Me>;

        // Initially no service should be running
        let running_status = Svc::already_running().await;
        assert!(running_status.is_none());

        // Register a service
        let (svc, _) = Svc::new(0).spawn().register().await.unwrap();

        // Service should be running
        let running_status = Svc::already_running().await;
        assert_eq!(running_status, Some(false)); // false means NOT stopped, so it's running

        // Stop the service
        let mut svc = svc;
        svc.stop().unwrap();
        svc.await.unwrap();

        // Service should now be stopped
        let running_status = Svc::already_running().await;
        assert_eq!(running_status, Some(true)); // true means stopped

        // Unregister the stopped service
        Svc::unregister().await;

        // No service should be in registry
        let running_status = Svc::already_running().await;
        assert!(running_status.is_none());
    }
}
