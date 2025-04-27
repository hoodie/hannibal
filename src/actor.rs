use std::future::Future;

use crate::context::Context;

mod build;

mod builder;
mod handle;
pub mod service;
pub mod spawnable;

pub(crate) mod restart_strategy;
pub use build::build;
pub(crate) use handle::{ActorHandle, JoinFuture};
pub use restart_strategy::RestartableActor;

/// Convenience type alias for `Box<dyn std::error::Error + Send + Sync>`.
pub type DynResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// An actor is an object that can receive messages.
pub trait Actor: Sized + Send + 'static {
    /// The name of the actor.
    ///
    /// This can be used for logging and debugging purposes, for instance in .
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

    /// Called when the actor is stopped.
    ///
    /// This asynchronous method is used for clean-up logic after the actor has finished processing messages.
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

    use std::sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering},
    };

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
