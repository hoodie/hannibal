use std::future::Future;

use crate::context::Context;

#[cfg(any(feature = "tokio", feature = "async-std"))]
mod build;

#[cfg(any(feature = "tokio", feature = "async-std"))]
mod builder;
pub mod service;
pub mod spawner;

pub(crate) mod restart_strategy;
#[cfg(any(feature = "tokio", feature = "async-std"))]
pub use build::build;
pub use restart_strategy::RestartableActor;

pub type DynResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// An actor is an object that can receive messages.
pub trait Actor: Sized + Send + 'static {
    const NAME: &'static str = "hannibal::Actor";

    /// Called when the actor is started.
    ///
    /// This method is async, the receiving of the first message is delayed until this method
    /// has completed.
    /// Returning an error will stop the actor.
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
        type Response = Pong;
    }
    pub struct Identify;
    impl crate::Message for Identify {
        type Response = usize;
    }

    #[cfg(feature = "async-std")]
    pub mod spawned_with_asyncstd {
        use std::marker::PhantomData;

        use super::{Identify, Ping, Pong};
        use crate::{
            Handler, Service,
            actor::{Actor, Context},
        };

        #[derive(Debug, Default)]
        pub struct AsyncStdActor<T: Send + Sync + Default>(pub usize, pub PhantomData<T>);

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
            LazyLock,
            atomic::{AtomicUsize, Ordering},
        };

        use super::{Identify, Ping, Pong};
        use crate::{
            Handler, Service,
            actor::{Actor, Context},
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
