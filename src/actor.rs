use std::future::Future;

use crate::context::Context;

pub mod service;
pub mod spawn_strategy;

pub type DynResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub trait Actor: Sized + Send + 'static {
    #[allow(unused)]
    fn started(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = DynResult> + Send {
        async { Ok(()) }
    }

    #[allow(unused)]
    fn stopped(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async {}
    }
}

#[cfg(test)]
pub mod tests {
    pub struct Ping;
    pub struct Pong;
    impl crate::Message for Ping {
        type Result = Pong;
    }
    pub struct Identify;
    impl crate::Message for Identify {
        type Result = usize;
    }

    #[cfg(feature = "async-std")]
    pub mod spawned_with_asyncstd {
        use super::{Identify, Ping, Pong};
        use crate::{
            actor::{spawn_strategy::AsyncStdSpawner, Actor, Context},
            Handler, SpawnableService,
        };

        #[derive(Debug, Default)]
        pub struct AsyncStdActor(pub usize);
        impl Actor for AsyncStdActor {}
        impl Service<AsyncStdSpawner> for AsyncStdActor {}
        impl Handler<Ping> for AsyncStdActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) -> Pong {
                Pong
            }
        }
        impl Handler<Identify> for AsyncStdActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Identify) -> usize {
                self.0
            }
        }
    }

    #[cfg(feature = "tokio")]
    pub mod spawned_with_tokio {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            LazyLock,
        };

        use super::{Identify, Ping, Pong};
        use crate::{
            actor::{spawn_strategy::TokioSpawner, Actor, Context},
            Handler, SpawnableService,
        };

        #[derive(Debug)]
        pub struct TokioActor(pub usize);

        impl Default for TokioActor {
            fn default() -> Self {
                static COUNTER: LazyLock<AtomicUsize> = LazyLock::new(Default::default);
                Self(COUNTER.fetch_add(1, Ordering::Relaxed))
            }
        }

        impl Actor for TokioActor {}
        impl SpawnableService<TokioSpawner> for TokioActor {}
        impl Handler<Ping> for TokioActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) -> Pong {
                Pong
            }
        }
        impl Handler<Identify> for TokioActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Identify) -> usize {
                self.0
            }
        }
    }
}
