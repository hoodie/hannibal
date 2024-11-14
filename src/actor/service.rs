use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::LazyLock,
};

use futures::FutureExt as _;

use super::{spawn_strategy::Spawner, *};

use crate::{environment::Environment, Addr};

type AnyBox = Box<dyn Any + Send + Sync>;

static REGISTRY: LazyLock<async_lock::RwLock<HashMap<TypeId, AnyBox>>> =
    LazyLock::new(Default::default);

#[cfg(any(feature = "tokio", feature = "async-std"))]
impl<A: Service> Addr<A> {
    /// will not replace a running service
    pub async fn register(self) -> crate::error::Result<(Self, Option<Self>)> {
        let key = TypeId::of::<A>();
        let mut registry = REGISTRY.write().await;

        let replaced = if let Some(existing) = registry.get(&key) {
            if existing
                .downcast_ref::<Addr<A>>()
                .map_or(false, Addr::stopped)
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

    pub async fn replace(self) -> Option<Self> {
        let key = TypeId::of::<A>();
        let mut registry = REGISTRY.write().await;
        registry
            .insert(key, Box::new(self.clone()))
            .and_then(|addr| addr.downcast::<Addr<A>>().ok())
            .map(|addr| *addr)
    }

    pub async fn unregister() -> Option<Addr<A>> {
        let key = TypeId::of::<A>();
        let mut registry = REGISTRY.write().await;
        registry
            .remove(&key)
            .and_then(|addr| addr.downcast::<Addr<A>>().ok())
            .map(|addr| *addr)
    }
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub trait Service: Actor + Default {
    fn setup() -> impl Future<Output = DynResult<()>> {
        Self::from_registry_and_spawn().map(|_| Ok(()))
    }

    fn already_running() -> impl Future<Output = Option<bool>> {
        async {
            let key = TypeId::of::<Self>();
            let registry = REGISTRY.read().await;
            registry
                .get(&key)
                .and_then(|addr| addr.downcast_ref::<Addr<Self>>().map(Addr::stopped))
        }
    }

    fn from_registry() -> impl Future<Output = Addr<Self>> {
        Self::from_registry_and_spawn()
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
                let (event_loop, addr) = Environment::unbounded().launch(Self::default());
                S::spawn_actor(event_loop);
                registry.insert(key, Box::new(addr.clone()));
                addr
            }
        }
    }
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub trait SpawnableService<S: Spawner<Self>>: Service {
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
                let (event_loop, addr) = Environment::unbounded().launch(Self::default());
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
    A: spawn_strategy::Spawnable<S>,
    S: Spawner<A>,
{
}

#[cfg(test)]
mod tests {

    #[cfg(feature = "tokio")]
    mod spawned_with_tokio {
        #![allow(clippy::unwrap_used)]
        use crate::{
            actor::tests::{spawned_with_tokio::TokioActor, Identify, Ping},
            prelude::Spawnable as _,
            spawn_strategy::{SpawnableWith, TokioSpawner},
            Service,
        };

        #[tokio::test]
        async fn register_as_service() {
            type Svc = TokioActor<u32>;
            let (addr, mut joiner) = Svc::new(1337).spawn_with::<TokioSpawner>().unwrap();
            let (mut addr, _) = addr.register().await.unwrap();
            assert_eq!(addr.call(Identify).await.unwrap(), 1337);
            assert_eq!(addr.call(Identify).await.unwrap(), 1337);
            addr.stop().unwrap();
            joiner.join().await.unwrap();
        }

        #[tokio::test]
        async fn get_service_from_registry() {
            type Svc = TokioActor<u64>;
            let mut svc_addr = Svc::from_registry().await;
            assert!(!svc_addr.stopped());

            svc_addr.call(Ping).await.unwrap();

            svc_addr.stop().unwrap();
            svc_addr.await.unwrap();
        }

        #[tokio::test]
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
            actor::tests::{spawned_with_asyncstd::AsyncStdActor, Identify, Ping},
            spawn_strategy::{AsyncStdSpawner, SpawnableWith},
            Service,
        };

        #[async_std::test]
        async fn register_as_service() {
            type Svc = AsyncStdActor<u32>;
            let (addr, mut joiner) = Svc::new(1337).spawn_with::<AsyncStdSpawner>().unwrap();
            addr.register().await.unwrap();
            let mut svc_addr = Svc::from_registry().await;
            assert_eq!(svc_addr.call(Identify).await.unwrap(), 1337);
            assert_eq!(svc_addr.call(Identify).await.unwrap(), 1337);

            svc_addr.stop().unwrap();
            joiner.join().await.unwrap();
        }

        #[tokio::test]
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
