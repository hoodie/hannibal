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

    use crate::actor::service::Service;
    use spawn_strategy::{AsyncStdSpawner, DefaultSpawnable, TokioSpawner};

    use crate::{Handler, Message};

    use super::*;

    #[tokio::test]
    async fn get_service_from_registry() {
        struct Ping;
        struct Pong;
        impl Message for Ping {
            type Result = Pong;
        }

        #[derive(Debug, Default)]
        struct AsyncStdServiceActor;
        impl Actor for AsyncStdServiceActor {}
        impl Service<AsyncStdSpawner> for AsyncStdServiceActor {}
        impl Handler<Ping> for AsyncStdServiceActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) -> Pong {
                Pong
            }
        }

        #[derive(Debug, Default)]
        struct TokioServiceActor;
        impl Actor for TokioServiceActor {}
        impl Service<TokioSpawner> for TokioServiceActor {}
        impl Handler<Ping> for TokioServiceActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) -> Pong {
                Pong
            }
        }

        let mut svc_addr_asyncstd = AsyncStdServiceActor::from_registry().await.unwrap();
        assert!(!svc_addr_asyncstd.stopped());

        svc_addr_asyncstd.call(Ping).await.unwrap();
        svc_addr_asyncstd.stop().unwrap();
        svc_addr_asyncstd.await.unwrap();

        let mut addr_tokio = <TokioServiceActor as DefaultSpawnable<TokioSpawner>>::spawn_default()
            .unwrap()
            .0;
        assert!(!addr_tokio.stopped());
        addr_tokio.call(Ping).await.unwrap();
        addr_tokio.stop().unwrap();
        addr_tokio.await.unwrap();

        let mut svc_addr_tokio = TokioServiceActor::from_registry().await.unwrap();
        assert!(!svc_addr_tokio.stopped());

        svc_addr_tokio.call(Ping).await.unwrap();
        svc_addr_tokio.stop().unwrap();
        svc_addr_tokio.await.unwrap()
    }
}
