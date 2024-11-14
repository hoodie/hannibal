use std::{future::Future, marker::PhantomData, time::Duration};

use futures::{channel::oneshot, FutureExt as _, Stream, StreamExt as _};

use crate::{
    actor::{
        restart_strategy::{RecreateFromDefault, RestartOnly, RestartStrategy},
        Actor,
    },
    channel::{ChanRx, Channel},
    context::StopNotifier,
    handler::StreamHandler,
    Addr, Context,
};

mod payload;
pub(crate) use payload::Payload;

#[derive(Debug, Default)]
pub struct EnvironmentConfig {
    pub timeout: Option<Duration>,
    pub fail_on_timeout: bool,
}

pub struct Environment<A: Actor, R: RestartStrategy<A> = RestartOnly> {
    ctx: Context<A>,
    addr: Addr<A>,
    stop: StopNotifier,
    config: EnvironmentConfig,
    payload_rx: ChanRx<A>,
    phantom: PhantomData<R>,
}

impl<A: Actor, R: RestartStrategy<A>> Environment<A, R> {
    pub(crate) fn from_channel(channel: Channel<A>) -> Self {
        let (tx_running, rx_running) = oneshot::channel::<()>();
        let ctx = Context {
            weak_tx: channel.weak_tx(),
            running: futures::FutureExt::shared(rx_running),
            children: Default::default(),
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
            config: Default::default(),
            phantom: PhantomData,
        }
    }
    pub(crate) const fn with_config(mut self, config: EnvironmentConfig) -> Self {
        self.config = config;
        self
    }
}

impl<A: Actor> Environment<A> {
    pub fn bounded(capacity: usize) -> Self {
        Self::from_channel(Channel::bounded(capacity))
    }

    pub fn unbounded() -> Self {
        Self::from_channel(Channel::unbounded())
    }
}

// TODO: consider dynamically deciding timeout based on `Task` vs `DeadlineTask
async fn timeout_fut(
    fut: impl Future<Output = ()>,
    timeout: Option<Duration>,
) -> crate::DynResult<()> {
    if let Some(timeout) = timeout {
        futures::select! {
            res = fut.map(Ok).fuse() => res,
            _ = futures_timer::Delay::new(timeout).fuse() => Err(crate::error::ActorError::Timeout.into())
        }
    } else {
        fut.map(Ok).await
    }
}

impl<A: Actor, R: RestartStrategy<A>> Environment<A, R> {
    pub fn launch(mut self, mut actor: A) -> (impl Future<Output = crate::DynResult<A>>, Addr<A>) {
        let actor_loop = async move {
            actor.started(&mut self.ctx).await?;

            let timeout = self.config.timeout;
            while let Some(event) = self.payload_rx.recv().await {
                match event {
                    Payload::Restart => actor = R::refresh(actor, &mut self.ctx).await?,
                    Payload::Task(f) => {
                        if let Err(err) = timeout_fut(f(&mut actor, &mut self.ctx), timeout).await {
                            if self.config.fail_on_timeout {
                                // log::warn!("actor task took too long: {:?}, exiting", err);
                                return Err(err);
                            } else {
                                // log::warn!("actor task took too long: {:?}, ignoring", err);
                                continue;
                            }
                        }
                    }

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
    ) -> (impl Future<Output = crate::DynResult<A>>, Addr<A>)
    where
        S: Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: StreamHandler<S::Item>,
    {
        let actor_loop = async move {
            actor.started(&mut self.ctx).await?;
            loop {
                futures::select! {
                    actor_msg = self.payload_rx.recv().fuse() => {
                        match actor_msg {
                            Some(Payload::Task(f)) => f(&mut actor, &mut self.ctx).await,
                            Some(Payload::Stop)  =>  break,
                            Some(Payload::Restart)  =>  {
                                panic!("restart message in streamhandling actor")
                                // TODO: what does this do with the
                                // log::warn!("ignoring restart message in streamhandling actor")
                            },
                            None =>  break
                        }
                    },
                    stream_msg = stream.next().fuse() => {
                        let Some(msg) = stream_msg else {
                            // stream is done, actor is done
                            break
                        };
                        StreamHandler::handle(&mut actor, &mut self.ctx, msg).await;
                    },
                    complete => break,
                    // default => break, // TODO: should this be here?
                }
            }

            actor.finished(&mut self.ctx).await;
            actor.stopped(&mut self.ctx).await;

            self.stop.notify();
            Ok(actor)
        };

        (actor_loop, self.addr)
    }
}

impl<A: Actor, R: RestartStrategy<A>> Environment<A, R>
where
    A: Default,
{
    pub fn recreating(self) -> Environment<A, RecreateFromDefault> {
        Environment {
            ctx: self.ctx,
            addr: self.addr,
            stop: self.stop,
            config: self.config,
            payload_rx: self.payload_rx,
            phantom: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{error::ActorError, Actor, DynResult};

    #[derive(Default)]
    struct GoodActor {
        started: bool,
        stopped: bool,
        count: i32,
    }

    impl Actor for GoodActor {
        async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult {
            self.started = true;
            Ok(())
        }

        async fn stopped(&mut self, _: &mut Context<Self>) {
            self.stopped = true;
        }
    }

    impl StreamHandler<i32> for GoodActor {
        async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
            self.count += msg;
        }
    }

    #[derive(Default, Debug)]
    pub struct BadActor;
    impl Actor for BadActor {
        async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult {
            Err(String::from("failed").into())
        }
    }

    impl StreamHandler<i32> for BadActor {
        async fn handle(&mut self, _: &mut Context<Self>, _: i32) {
            unreachable!()
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

        fn prepare<A>() -> (impl Future<Output = DynResult<A>>, Addr<A>)
        where
            A: Actor + Default + 'static,
            A: StreamHandler<i32>,
        {
            let counter = futures::stream::iter(0..100);
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

    mod restart {
        use super::*;
        use crate::RestartableActor;

        #[derive(Debug, Default)]
        struct RestartCounter {
            started_count: usize,
            stopped_count: usize,
        }

        impl RestartCounter {
            const fn new() -> Self {
                Self {
                    started_count: 0,
                    stopped_count: 0,
                }
            }
        }

        impl RestartableActor for RestartCounter {}
        impl Actor for RestartCounter {
            async fn started(&mut self, _: &mut Context<Self>) -> DynResult {
                self.started_count += 1;
                eprintln!("started: {:?}", self);
                Ok(())
            }

            async fn stopped(&mut self, _: &mut Context<Self>) {
                self.stopped_count += 1;
                eprintln!("stopped: {:?}", self);
            }
        }

        #[tokio::test]
        async fn restarts_actor() {
            let counter = RestartCounter::new();
            let (event_loop, mut addr) = Environment::unbounded().launch(counter);
            let task = tokio::spawn(event_loop);
            addr.restart().unwrap();
            addr.restart().unwrap();
            addr.stop().unwrap();
            let actor = task.await.unwrap().unwrap();
            assert_eq!(actor.started_count, 3);
            assert_eq!(actor.stopped_count, 3);
        }

        #[tokio::test]
        async fn recreates_actor() {
            let counter = RestartCounter::new();
            let (event_loop, mut addr) = Environment::unbounded().recreating().launch(counter);
            let task = tokio::spawn(event_loop);
            addr.restart().unwrap();
            addr.restart().unwrap();
            addr.stop().unwrap();
            let actor = task.await.unwrap().unwrap();
            assert_eq!(actor.started_count, 1);
            assert_eq!(actor.stopped_count, 1, "should only be stopped once");
        }
    }

    #[cfg(any(feature = "tokio", feature = "async-std"))]
    mod timeout {

        use std::time::Duration;

        use crate::{error::ActorError, prelude::*, RestartableActor};

        cfg_if::cfg_if! {
        if #[cfg(feature = "async-std")] {
            use async_std::task::sleep;
        } else {
            use tokio::time::sleep;
        }
        }

        #[derive(Debug, Default)]
        struct SleepyActor(u8);

        struct Sleep(Duration);
        impl Message for Sleep {
            type Result = ();
        }

        impl Actor for SleepyActor {
            async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult<()> {
                println!("[ SleepyActor {} ] started", self.0);
                Ok(())
            }

            async fn stopped(&mut self, _ctx: &mut Context<Self>) {
                println!("[ SleepyActor {} ] stopped", self.0);
            }
        }

        impl Handler<Sleep> for SleepyActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Sleep) {
                println!("[ SleepyActor {} ] is resting for {:?}", self.0, msg.0);
                sleep(msg.0).await;
                println!("[ SleepyActor {} ] woke up after {:?}", self.0, msg.0);
            }
        }

        // TODO: can we encode the restart strategy in an associated type or as a trait function?
        impl RestartableActor for SleepyActor {}

        #[tokio::test]
        async fn no_timeout() {
            // normal case, tasks take long
            println!("SleepyActor 0 will take 1 second to complete");
            let mut addr = crate::build(SleepyActor(0))
                .bounded(1)
                .recreate_from_default()
                .spawn();
            addr.call(Sleep(Duration::from_millis(100))).await.unwrap();
            assert!(addr.stop().is_ok());
        }

        #[tokio::test]
        async fn timeout_and_continue() {
            // timeout and continue
            println!("SleepyActor 1 will be canceled after 1 second");
            println!("SleepyActor 1 still accepts messages after being canceled");
            let mut addr = crate::build(SleepyActor(1))
                .bounded(1)
                .timeout(Duration::from_millis(100))
                .fail_on_timeout(false)
                .recreate_from_default()
                .spawn_owning();
            assert!(addr
                .as_ref()
                .call(Sleep(Duration::from_secs(0)))
                .await
                .is_ok());
            assert!(matches!(
                addr.as_addr()
                    .call(Sleep(Duration::from_secs(4)))
                    .await
                    .unwrap_err(),
                ActorError::Canceled(_)
            ));
            assert!(addr
                .as_addr()
                .call(Sleep(Duration::from_secs(0)))
                .await
                .is_ok());
            eprintln!("SleepyActor 2 is still alive, stopping");
            assert!(addr.to_addr().stop().is_ok());
            assert!(addr.join().await.is_some());
        }

        #[tokio::test]
        async fn timeout_and_fail() {
            // timeout and fail
            println!("SleepyActor 2 will be canceled after 1 second");
            let mut addr = crate::build(SleepyActor(2))
                .bounded(1)
                .timeout(Duration::from_millis(100))
                .fail_on_timeout(true)
                .recreate_from_default()
                .spawn_owning();
            assert!(addr
                .as_ref()
                .call(Sleep(Duration::from_secs(60)))
                .await
                .is_err());
            println!("SleepyActor 2 nolonger accepts messages after being canceled");
            assert!(addr
                .as_ref()
                .call(Sleep(Duration::from_secs(0)))
                .await
                .is_err());
            assert!(addr.join().await.is_none());
            assert!(addr.to_addr().stop().is_err());
        }
    }
}
