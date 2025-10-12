use std::{future::Future, marker::PhantomData, pin::pin, time::Duration};

use futures::{FutureExt, Stream, StreamExt as _, channel::oneshot};

use crate::{
    Actor, Addr, Context,
    actor::restart_strategy::{RecreateFromDefault, RestartOnly, RestartStrategy},
    channel::{Channel, Rx},
    context::StopNotifier,
    handler::StreamHandler,
    runtime::sleep,
};

mod payload;
pub(crate) use payload::Payload;

#[derive(Debug, Default)]
pub struct EventLoopConfig {
    pub timeout: Option<Duration>,
    pub fail_on_timeout: bool,
}

pub struct EventLoop<A: Actor, R: RestartStrategy<A> = RestartOnly> {
    ctx: Context<A>,
    addr: Addr<A>,
    stop: StopNotifier,
    config: EventLoopConfig,
    rx: Rx<A>,
    phantom: PhantomData<R>,
}

impl<A: Actor, R: RestartStrategy<A>> EventLoop<A, R> {
    pub(crate) fn from_channel(channel: Channel<A>) -> Self {
        let (tx_running, rx_running) = oneshot::channel::<()>();
        let (tx, rx) = channel.break_up();
        let weak_tx = tx.downgrade();
        let ctx = Context {
            id: Default::default(),
            weak_tx,
            running: futures::FutureExt::shared(rx_running),
            children: Default::default(),
            tasks: Default::default(),
        };
        let stop = StopNotifier(tx_running);

        let addr = Addr {
            context_id: ctx.id,
            tx,
            running: ctx.running.clone(),
        };
        EventLoop {
            ctx,
            addr,
            stop,
            rx,
            config: Default::default(),
            phantom: PhantomData,
        }
    }

    pub(crate) const fn with_config(mut self, config: EventLoopConfig) -> Self {
        self.config = config;
        self
    }
}

impl<A: Actor> EventLoop<A> {
    pub fn bounded(capacity: usize) -> Self {
        Self::from_channel(Channel::bounded(capacity))
    }

    pub fn unbounded() -> Self {
        Self::from_channel(Channel::unbounded())
    }

    pub fn abort_after(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout.into();
        self.config.fail_on_timeout = false;
        self
    }

    pub fn fail_after(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout.into();
        self.config.fail_on_timeout = true;
        self
    }
}

// TODO: consider dynamically deciding timeout based on `Task` vs `DeadlineTask
pub async fn timeout_fut(
    fut: impl Future<Output = ()>,
    timeout: Option<Duration>,
) -> crate::DynResult<()> {
    if let Some(timeout) = timeout {
        futures::select! {
            res = fut.map(Ok).fuse() => res,
            _ = FutureExt::fuse(sleep(timeout)) => Err(crate::error::ActorError::Timeout.into())
        }
    } else {
        fut.map(Ok).await
    }
}

impl<A: Actor, R: RestartStrategy<A>> EventLoop<A, R> {
    pub fn create(mut self, mut actor: A) -> (impl Future<Output = crate::DynResult<A>>, Addr<A>) {
        let actor_loop = async move {
            actor.started(&mut self.ctx).await?;

            let timeout = self.config.timeout;
            #[cfg(feature = "async_channel")]
            log::trace!(actor=A::NAME, id:% =self.ctx.id; "still waiting for {} events", self.rx.len());
            let mut payload_rx = pin!(self.rx);
            while let Some(event) = payload_rx.next().await {
                log::trace!(actor=A::NAME, id:% =self.ctx.id; "processing event");
                match event {
                    Payload::Restart => {
                        log::trace!(actor=A::NAME, id:% =self.ctx.id; "restarting");
                        actor = R::refresh(actor, &mut self.ctx).await?
                    }
                    Payload::Task(f) => {
                        log::trace!(actor=A::NAME, id:% =self.ctx.id;  "received task");
                        if let Err(err) = timeout_fut(f(&mut actor, &mut self.ctx), timeout).await {
                            if self.config.fail_on_timeout {
                                log::warn!(actor=A::NAME, id:% =self.ctx.id; "{}, exiting", err);
                                actor.cancelled(&mut self.ctx).await;
                                return Err(err);
                            } else {
                                log::warn!(actor=A::NAME, id:% =self.ctx.id; "{}, ignoring", err);
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

    pub fn create_on_stream<S>(
        mut self,
        mut actor: A,
        mut stream: S,
    ) -> (impl Future<Output = crate::DynResult<A>>, Addr<A>)
    where
        S: Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: StreamHandler<S::Item>,
    {
        let timeout = self.config.timeout;
        let actor_loop = async move {
            actor.started(&mut self.ctx).await?;
            let mut rx = pin!(self.rx);
            loop {
                futures::select! {
                    event = rx.next().fuse() => {
                        match event {
                            Some(Payload::Task(task_fn)) => {
                                log::trace!(name = A::NAME;  "received task");
                                if let Err(err) = timeout_fut(task_fn(&mut actor, &mut self.ctx), timeout).await {
                                    if self.config.fail_on_timeout {
                                        log::warn!("{} {}, exiting", A::NAME, err);
                                        actor.cancelled(&mut self.ctx).await;
                                        return Err(err);
                                    } else {
                                        log::warn!("{} {}, ignoring", A::NAME, err);
                                    }
                                }
                            }
                            Some(Payload::Stop)  =>  break,
                            Some(Payload::Restart)  =>  {
                                panic!("restart message in stream-handling actor")
                                // TODO: what does this do with the
                                // log::warn!("ignoring restart message in stream-handling actor")
                            },
                            None =>  break
                        }
                    },
                    stream_msg = stream.next().fuse() => {
                        let Some(msg) = stream_msg else {
                            actor.finished(&mut self.ctx).await;
                            break;
                        };
                        log::trace!(name = A::NAME;  "received stream message");
                        if let Err(err) = timeout_fut(
                            StreamHandler::handle(&mut actor, &mut self.ctx, msg) , timeout).await {
                            if self.config.fail_on_timeout {
                                log::warn!("{} {}, exiting", A::NAME, err);
                                actor.cancelled(&mut self.ctx).await;
                                return Err(err);
                            } else {
                                log::warn!("{} {}, ignoring", A::NAME, err);
                                continue;
                            }
                        }
                    },
                }
            }

            actor.stopped(&mut self.ctx).await;

            self.stop.notify();
            Ok(actor)
        };

        (actor_loop, self.addr)
    }
}

impl<A: Actor, R: RestartStrategy<A>> EventLoop<A, R>
where
    A: Default,
{
    pub fn recreating(self) -> EventLoop<A, RecreateFromDefault> {
        EventLoop {
            ctx: self.ctx,
            addr: self.addr,
            stop: self.stop,
            config: self.config,
            rx: self.rx,
            phantom: PhantomData,
        }
    }
}
