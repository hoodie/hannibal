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

    pub fn launch(
        mut self,
        mut actor: A,
    ) -> (impl Future<Output = crate::ActorResult<A>>, Addr<A>) {
        let actor_loop = async move {
            actor.started(&mut self.ctx).await?;

            while let Some(event) = self.payload_rx.recv().await {
                match event {
                    Payload::Task(f) => f(&mut actor, &mut self.ctx).await,
                    Payload::Stop => break,
                }
            }

            actor.stopped(&mut self.ctx).await;

            self.stop.notify();
            Ok(actor)
        };

        (actor_loop, self.addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actor, ActorResult};

    #[derive(Default)]
    struct MyActor {
        started: bool,
        stopped: bool,
    }

    impl Actor for MyActor {
        async fn started(&mut self, _ctx: &mut Context<Self>) -> ActorResult {
            self.started = true;
            Ok(())
        }

        async fn stopped(&mut self, _: &mut Context<Self>) {
            self.stopped = true;
        }
    }

    #[tokio::test]
    async fn calls_start() {
        let (event_loop, mut addr) = Environment::unbounded().launch(MyActor::default());
        let task = tokio::spawn(event_loop);
        addr.stop().unwrap();
        let actor = task.await.unwrap().unwrap();
        assert!(actor.started);
    }

    #[tokio::test]
    async fn calls_stop() {
        let (event_loop, mut addr) = Environment::unbounded().launch(MyActor::default());
        let task = tokio::spawn(event_loop);
        addr.stop().unwrap();
        let actor = task.await.unwrap().unwrap();
        assert!(actor.stopped);
    }

    #[derive(Debug)]
    struct BadActor;

    impl Actor for BadActor {
        async fn started(&mut self, _ctx: &mut Context<Self>) -> ActorResult {
            Err(String::from("failed").into())
        }
    }

    #[tokio::test]
    async fn start_can_fail() {
        let (event_loop, mut addr) = Environment::unbounded().launch(BadActor);
        let task = tokio::spawn(event_loop);
        addr.stop().unwrap();
        let error = task.await.unwrap().unwrap_err().to_string();
        assert_eq!(error, "failed");
    }
}
