mod addr;
mod channel;
mod context;
mod event_loop;
mod sender;

mod error;

pub use addr::Addr;
use channel::WeakChanTx;
pub use context::Context;
pub use event_loop::EventLoop;
pub use sender::Sender;

pub type ActorResult<T> = Result<T, error::ActorError>;

// pub mod non_blocking;
pub type RunningFuture = futures::future::Shared<futures::channel::oneshot::Receiver<()>>;

mod actor {
    use std::future::Future;

    use crate::{event_loop::StopReason, ActorResult, Context};

    pub trait Actor: Sized + Send + 'static {
        fn started(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = Result<()>> + Send {
            let _ = ctx;
        }
        fn stopping(
            &mut self,
            ctx: &mut Context<Self>,
            reason: StopReason,
        ) -> impl Future<Output = ()> + Send {
            async {}
        }

        fn stopped(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
            async {}
        }
    }
}

pub trait Actor: Send + Sync + 'static {
    fn started(&mut self) -> ActorResult<()>;
    fn stopped(&mut self) -> ActorResult<()>;
}

pub trait Handler<M>: Actor {
    fn handle(&mut self, msg: M);
}
