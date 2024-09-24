pub mod addr;
mod channel;
pub mod error;

pub use error::Result;

pub mod context {
    use futures::channel::oneshot;

    use crate::{
        actor::Actor,
        addr::Payload,
        channel::{ChannelWrapper, WeakChanTx},
        error::Result,
    };

    pub type RunningFuture = futures::future::Shared<oneshot::Receiver<()>>;
    pub struct StopNotifier(oneshot::Sender<()>);
    impl StopNotifier {
        pub fn notify(self) {
            self.0.send(()).ok();
        }
    }

    pub struct Context<A> {
        pub(crate) weak_tx: WeakChanTx<A>,
        pub(crate) running: RunningFuture,
    }

    impl<A: Actor> Context<A> {
        pub(crate) fn new(channel: &ChannelWrapper<A>) -> (Self, StopNotifier) {
            let (tx_running, rx_running) = oneshot::channel::<()>();
            let ctx = Self {
                weak_tx: channel.weak_tx(),
                running: futures::FutureExt::shared(rx_running),
            };

            (ctx, StopNotifier(tx_running))
        }

        pub fn stop(&self) -> Result<()> {
            if let Some(tx) = self.weak_tx.upgrade() {
                Ok(tx.send(Payload::Stop)?)
            } else {
                Err(crate::error::ActorError::AlreadyStopped)
            }
        }
    }
}

pub mod actor {
    use std::future::Future;

    use crate::{addr::Message, context::Context, error::Result};

    pub trait Actor: Sized + Send + 'static {
        #[allow(unused)]
        fn started(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = Result<()>> + Send {
            async { Ok(()) }
        }

        #[allow(unused)]
        fn stopped(&mut self, ctx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
            async {}
        }
    }

    pub trait Handler<M: Message>: Actor
    where
        Self: Sized,
    {
        fn handle(
            &mut self,
            ctx: &mut Context<Self>,
            msg: M,
        ) -> impl futures::Future<Output = M::Result> + Send;
    }
}
