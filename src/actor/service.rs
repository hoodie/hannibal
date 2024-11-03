use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::LazyLock,
};

use futures::FutureExt as _;

use super::{spawn_strategy::Spawner, *};

use crate::{environment::Environment, Addr};

type AnyBox = Box<dyn Any + Send + Sync>;

static REGISTRY: LazyLock<async_lock::Mutex<HashMap<TypeId, AnyBox>>> =
    LazyLock::new(Default::default);

#[cfg(any(feature = "tokio", feature = "async-std"))]
impl<A: Service> Addr<A> {
    pub async fn register(self) -> Option<Addr<A>> {
        let key = TypeId::of::<A>();
        let mut registry = REGISTRY.lock().await;

        let replaced = registry
            .insert(key, Box::new(self))
            .and_then(|addr| addr.downcast::<Addr<A>>().ok())
            .map(|addr| *addr);

        eprintln!("elements in registry after: {:?}", registry.iter().count());

        replaced
    }
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub trait Service: Actor + Default {
    fn setup() -> impl Future<Output = DynResult<()>> {
        Self::from_registry_and_spawn().map(|_| Ok(()))
    }

    fn from_registry() -> impl Future<Output = Addr<Self>> {
        Self::from_registry_and_spawn()
    }
}

#[cfg(not(any(feature = "tokio", feature = "async-std")))]
pub trait Service<S: Spawner<Self>>: Actor + Default + SpawnableService<S> {
    fn setup() -> impl Future<Output = DynResult<()>> {
        Self::from_registry_and_spawn()
            .map(|res| res.map(|_| ()))
            .map_err(Into::into)
    }

    fn from_registry() -> impl Future<Output = crate::error::Result<Addr<Self>>> {
        Self::from_registry_and_spawn()
    }
}

pub trait SpawnableService<S: Spawner<Self>>: Service {
    #[allow(clippy::async_yields_async)]
    fn from_registry_and_spawn() -> impl Future<Output = Addr<Self>> {
        async {
            let key = TypeId::of::<Self>();

            let mut registry = REGISTRY.lock().await;

            if let Some(addr) = registry
                .get_mut(&key)
                .and_then(|addr| addr.downcast_ref::<Addr<Self>>())
                .map(ToOwned::to_owned)
                .filter(|addr| !addr.stopped()) // TODO: test this
            {
                addr
            } else {
                #[allow(deprecated)]
                let (event_loop, addr) = Environment::unbounded().launch(Self::default());
                S::spawn(event_loop);
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
        use crate::{
            actor::tests::{spawned_with_tokio::TokioActor, Identify, Ping},
            spawn_strategy::{SpawnableWith, TokioSpawner},
            Service,
        };

        #[tokio::test]
        async fn register_as_service() {
            type Svc = TokioActor<u32>;
            let (addr, mut joiner) = Svc::new(1337).spawn_with::<TokioSpawner>().unwrap();
            addr.register().await;
            let mut svc_addr = Svc::from_registry().await;
            assert_eq!(svc_addr.call(Identify).await.unwrap(), 1337);
            assert_eq!(svc_addr.call(Identify).await.unwrap(), 1337);

            svc_addr.stop().unwrap();
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
            addr.register().await;
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
