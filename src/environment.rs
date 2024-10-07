use std::future::Future;

use futures::{channel::oneshot, FutureExt as _, StreamExt as _};

use crate::{
    actor::Actor,
    channel::{ChanRx, ChannelWrapper},
    context::StopNotifier,
    payload::Payload,
    Addr, Context, Handler, Message,
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

    pub fn launch_on_stream<S>(
        mut self,
        mut actor: A,
        mut stream: S,
    ) -> (impl Future<Output = crate::ActorResult<A>>, Addr<A>)
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: Message, // TODO: remove this bound
        S::Item: std::fmt::Debug,
        S::Item: 'static + Send,
        A: Handler<S::Item>,
    {
        let actor_loop = async move {
            actor.started(&mut self.ctx).await?;
            loop {
                futures::select! {
                    actor_msg = self.payload_rx.recv().fuse() => {
                        match actor_msg {
                            Some(Payload::Task(f)) => f(&mut actor, &mut self.ctx).await,
                            Some(Payload::Stop)  =>  break,
                            _ =>  break
                        }
                    },
                    stream_msg = stream.next().fuse() => {
                        let Some(msg) = stream_msg else {
                            // stream is done, actor is done
                            break
                        };
                        actor.handle(&mut self.ctx, msg).await;
                    },
                    complete => break,
                    default => break,
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
    use crate::{Actor, ActorError, ActorResult};

    #[derive(Default)]
    struct GoodActor {
        started: bool,
        stopped: bool,
        count: i32,
    }

    impl Actor for GoodActor {
        async fn started(&mut self, _ctx: &mut Context<Self>) -> ActorResult {
            self.started = true;
            Ok(())
        }

        async fn stopped(&mut self, _: &mut Context<Self>) {
            self.stopped = true;
        }
    }

    #[derive(Debug)]
    struct IncBy(i32);
    impl Message for IncBy {
        type Result = ();
    }
    impl Handler<IncBy> for GoodActor {
        async fn handle(&mut self, _ctx: &mut Context<Self>, msg: IncBy) {
            self.count += msg.0;
        }
    }

    #[derive(Default, Debug)]
    pub struct BadActor;
    impl Actor for BadActor {
        async fn started(&mut self, _ctx: &mut Context<Self>) -> ActorResult {
            Err(String::from("failed").into())
        }
    }

    impl Handler<IncBy> for BadActor {
        async fn handle(&mut self, _: &mut Context<Self>, _: IncBy) {
            // unreachable!()
        }
    }

    mod normal_start {
        use super::*;

        #[tokio::test]
        async fn calls_actor_started() {
            let (event_loop, mut addr) = Environment::unbounded().launch(GoodActor::default());
            let task = tokio::spawn(event_loop);
            addr.stop().unwrap();
            let actor = task.await.unwrap().unwrap();
            assert!(actor.started);
        }

        #[tokio::test]
        async fn calls_actor_stopped() {
            let (event_loop, mut addr) = Environment::unbounded().launch(GoodActor::default());
            let task = tokio::spawn(event_loop);
            addr.stop().unwrap();
            let actor = task.await.unwrap().unwrap();
            assert!(actor.stopped);
        }

        #[tokio::test]
        async fn start_can_fail() {
            let (event_loop, mut addr) = Environment::unbounded().launch(BadActor);
            let task = tokio::spawn(event_loop);
            addr.stop().unwrap();
            let error = task.await.unwrap().map(|_| ()).map_err(|e| e.to_string());
            assert_eq!(error, Err(String::from("failed")), "start should fail");
        }
    }

    mod start_on_stream {
        use super::*;

        fn prepare<A>() -> (impl Future<Output = ActorResult<A>>, Addr<A>)
        where
            A: Actor + Default + 'static,
            A: Handler<IncBy>,
        {
            let counter = futures::stream::iter(0..100).map(IncBy);
            Environment::unbounded().launch_on_stream(A::default(), counter)
        }

        #[tokio::test]
        async fn calls_actor_started() {
            let (event_loop, mut addr) = prepare::<GoodActor>();
            let task = tokio::spawn(event_loop);
            addr.stop().unwrap();
            let actor = task.await.unwrap().unwrap();
            assert!(actor.started);
        }

        #[tokio::test]
        async fn calls_actor_stopped() {
            let (event_loop, mut addr) = prepare::<GoodActor>();
            let task = tokio::spawn(event_loop);
            addr.stop().unwrap();
            let actor = task.await.unwrap().unwrap();
            assert!(actor.stopped);
        }

        #[tokio::test]
        async fn start_can_fail() {
            let (event_loop, mut addr) = prepare::<BadActor>();
            let task = tokio::spawn(event_loop);
            addr.stop().unwrap();
            let error = task.await.unwrap().map(|_| ()).map_err(|e| e.to_string());
            assert_eq!(error, Err(String::from("failed")), "start should fail");
        }

        #[tokio::test]
        async fn ends_on_stream_finish() {
            let (event_loop, mut addr) = prepare::<GoodActor>();
            let addr2 = addr.clone();

            let task = tokio::spawn(event_loop);
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;

            // TODO: should the stream always stop the actor?
            assert!(matches!(
                addr.stop().unwrap_err(),
                ActorError::AsyncSendError(_)
            ));
            assert!(
                addr2.await.is_ok(),
                "other address should be cleanly awaitable"
            );

            let actor = task.await.unwrap().unwrap();
            assert_eq!(actor.count, 4950);
        }
    }
}
