use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::LazyLock,
};

use super::spawn_strategy::Spawner;
use super::*;

use crate::{Addr, Environment};

type AnyBox = Box<dyn Any + Send + Sync>;

static REGISTRY: LazyLock<async_lock::Mutex<HashMap<TypeId, AnyBox>>> =
    LazyLock::new(Default::default);

pub trait Registerable<S>: Actor
where
    S: Spawner<Self>,
{
    fn register(instance: Addr<Self>) -> impl Future<Output = Option<Addr<Self>>> {
        async {
            let key = TypeId::of::<Self>();
            REGISTRY
                .lock()
                .await
                .insert(key, Box::new(instance))
                .and_then(|addr| addr.downcast::<Addr<Self>>().ok())
                .map(|addr| *addr)
        }
    }
}

pub trait Service<S>: Actor + Default + Registerable<S>
where
    S: Spawner<Self>,
{
    fn from_registry() -> impl Future<Output = crate::error::Result<Addr<Self>>> {
        async {
            let key = TypeId::of::<Self>();

            let mut entry = REGISTRY.lock().await;

            if let Some(addr) = entry
                .get_mut(&key)
                .and_then(|addr| addr.downcast_ref::<Addr<Self>>())
                .map(ToOwned::to_owned)
            {
                Ok(addr)
            } else {
                let (event_loop, addr) = Environment::unbounded().launch(Self::default());
                S::spawn(event_loop);
                entry.insert(key, Box::new(addr.clone()));
                Ok(addr)
            }
        }
    }
}

impl<T, S> Registerable<S> for T
where
    T: Service<S>,
    S: Spawner<Self>,
{
}

#[cfg(test)]
mod tests {

    #[cfg(feature = "tokio")]
    mod spawned_with_tokio {
        use crate::{
            actor::tests::{spawned_with_tokio::TokioActor, Ping},
            Service,
        };

        #[tokio::test]
        async fn get_service_from_registry() {
            let mut svc_addr = TokioActor::from_registry().await.unwrap();
            assert!(!svc_addr.stopped());

            svc_addr.call(Ping).await.unwrap();
            svc_addr.stop().unwrap();
            svc_addr.await.unwrap()
        }
    }

    #[cfg(feature = "async-std")]
    mod spawned_with_asyncstd {
        use crate::{
            actor::tests::{spawned_with_asyncstd::AsyncStdActor, Ping},
            Service,
        };

        #[async_std::test]
        async fn get_service_from_registry() {
            let mut svc_addr = AsyncStdActor::from_registry().await.unwrap();
            assert!(!svc_addr.stopped());

            svc_addr.call(Ping).await.unwrap();
            svc_addr.stop().unwrap();
            svc_addr.await.unwrap();
        }
    }
}
