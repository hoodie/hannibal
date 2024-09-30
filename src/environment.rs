use std::future::Future;

use futures::channel::oneshot;

use crate::{
    actor::Actor,
    addr::Payload,
    channel::{ChanRx, ChannelWrapper},
    context::StopNotifier,
    Addr, Context,
};

pub struct Environment<A: Actor> {
    ctx: Context<A>,
    addr: Addr<A>,
    stop: StopNotifier,
    payload_rx: ChanRx<A>,
}

impl<A: Actor> Environment<A> {
    pub fn bounded(capacity: usize) -> Self {
        Self::from_channel(ChannelWrapper::bounded(capacity))
    }

    pub fn unbounded() -> Self {
        Self::from_channel(ChannelWrapper::unbounded())
    }

    fn from_channel(channel: ChannelWrapper<A>) -> Self {
        let (tx_running, rx_running) = oneshot::channel::<()>();
        let ctx = Context {
            weak_tx: channel.weak_tx(),
            running: futures::FutureExt::shared(rx_running),
        };
        let (payload_tx, payload_rx) = channel.break_up();
        let stop = StopNotifier(tx_running);

        let addr = Addr {
            payload_tx,
            running: ctx.running.clone(),
        };
        Environment {
            ctx,
            addr,
            stop,
            payload_rx,
        }
    }

    pub fn launch(mut self, mut actor: A) -> (impl Future<Output = A>, Addr<A>) {
        let actor_loop = async move {
            while let Some(event) = self.receiver.recv().await {
                match event {
                    Payload::Task(f) => f(&mut actor, &mut self.ctx).await,
                    Payload::Stop => break,
                }
            }

            actor.stopped(&mut self.ctx).await;

            self.stop.notify();
            actor
        };

        (actor_loop, self.addr)
    }
}
