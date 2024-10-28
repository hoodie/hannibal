use std::future::Future;

use crate::context::Context;

pub mod service;
pub mod spawn_strategy;

pub mod restart_strategy;
pub use restart_strategy::RestartableActor;
pub(crate) use restart_strategy::{NonRestartable, RecreateFromDefault, RestartOnly};

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
            actor::{Actor, Context},
            Handler, Service,
        };

        #[derive(Debug, Default)]
        pub struct AsyncStdActor<T: Send + Sync + Default>(
            pub usize,
            pub std::marker::PhantomData<T>,
        );

        impl<T: Send + Sync + Default> AsyncStdActor<T> {
            pub fn new(value: usize) -> Self {
                Self(value, Default::default())
            }
        }

        impl<T: Send + Sync + Default + 'static> Actor for AsyncStdActor<T> {}
        impl<T: Send + Sync + Default + 'static> Service for AsyncStdActor<T> {}
        impl<T: Send + Sync + Default + 'static> Handler<Ping> for AsyncStdActor<T> {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) -> Pong {
                Pong
            }
        }
        impl<T: Send + Sync + Default + 'static> Handler<Identify> for AsyncStdActor<T> {
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
            actor::{Actor, Context},
            Handler, Service,
        };

        #[derive(Debug)]
        pub struct TokioActor<T: Send + Sync + Default>(pub usize, pub std::marker::PhantomData<T>);

        impl<T: Send + Sync + Default> TokioActor<T> {
            pub fn new(value: usize) -> Self {
                Self(value, Default::default())
            }
        }

        impl<T: Send + Sync + Default> Default for TokioActor<T> {
            fn default() -> Self {
                static COUNTER: LazyLock<AtomicUsize> = LazyLock::new(Default::default);
                Self(COUNTER.fetch_add(1, Ordering::Relaxed), Default::default())
            }
        }

        impl<T: Send + Sync + Default + 'static> Actor for TokioActor<T> {}
        impl<T: Send + Sync + Default + 'static> Service for TokioActor<T> {}
        impl<T: Send + Sync + Default + 'static> Handler<Ping> for TokioActor<T> {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) -> Pong {
                eprintln!("TokioActor({}): Ping", self.0);
                Pong
            }
        }
        impl<T: Send + Sync + Default + 'static> Handler<Identify> for TokioActor<T> {
            async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Identify) -> usize {
                self.0
            }
        }
    }
}
