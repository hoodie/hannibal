use std::{borrow::Cow, future::Future};

use crate::{
    addr::ActorEvent,
    channel::{ChanRx, ChannelWrapper},
    error::Result,
    Actor, Addr, Context,
};

use futures::{FutureExt, StreamExt};

pub(crate) struct LifeCycle<A: Actor> {
    ctx: Context<A>,
    addr: Addr<A>,
    // channel: ChannelWrapper<A>,
    rx: ChanRx<A>,
    tx_exit: futures::channel::oneshot::Sender<()>,
}

impl<A: Actor> LifeCycle<A> {
    pub(crate) fn new() -> Self {
        let mut channel = ChannelWrapper::unbounded();
        let (tx_exit, rx_exit) = futures::channel::oneshot::channel::<()>();
        let ctx = Context::new(rx_exit.shared().into(), &channel);
        let addr = Addr {
            actor_id: ctx.actor_id(),
            tx: channel.tx(),
            rx_exit: ctx.rx_exit.clone(),
        };
        let rx = channel.rx().take().unwrap();

        Self {
            ctx,
            addr,
            rx,
            tx_exit,
        }
    }

    pub(crate) fn new_bounded(buffer: usize) -> Self {
        let mut channel = ChannelWrapper::bounded(buffer);
        let (tx_exit, rx_exit) = futures::channel::oneshot::channel::<()>();
        let ctx = Context::new(rx_exit.shared().into(), &channel);
        let addr = Addr {
            actor_id: ctx.actor_id(),
            tx: channel.tx(),
            rx_exit: ctx.rx_exit.clone(),
        };
        let rx = channel.rx().take().unwrap();
        Self {
            ctx,
            addr,
            rx,
            tx_exit,
        }
    }
}

impl<A: Actor> LifeCycle<A> {
    pub(crate) async fn cycle(
        self,
        mut actor: A,
    ) -> Result<(impl Future<Output = ()>, Addr<A>, Cow<'static, str>)> {
        let Self {
            mut ctx,
            addr,
            mut rx,
            tx_exit,
        } = self;

        // 1. tell actor that it started
        Actor::started(&mut actor, &mut ctx).await?;
        let actor_name = actor.name();

        let actor_loop = async move {
            // 2. configure actor loop
            while let Some(event) = rx.recv().await {
                match event {
                    ActorEvent::Exec(f) => f(&mut actor, &mut ctx).await,
                    ActorEvent::Stop(reason) => {
                        actor.stopping(&mut ctx, reason).await;
                        break;
                    }
                    ActorEvent::StopSupervisor(_err) => {}
                    ActorEvent::RemoveStream(id) => {
                        if ctx.streams.contains(id) {
                            ctx.streams.remove(id);
                        }
                    }
                }
            }

            // 3. abort streams and intervals
            ctx.abort_streams_and_intervals();

            // 4. tell actor that it stopped
            actor.stopped(&mut ctx).await;

            // 5. send exit signal
            tx_exit.send(()).ok();
        };

        Ok((actor_loop, addr, actor_name))
    }

    async fn stream_cycle<S>(
        self,
        mut actor: A,
        stream: S,
    ) -> Result<(impl Future<Output = ()>, Addr<A>, Cow<'static, str>)>
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: crate::StreamHandler<S::Item>,
    {
        let Self {
            mut ctx,
            addr,
            mut rx,
            tx_exit,
        } = self;

        ctx.add_stream(stream);

        // 1. tell actor that it started
        Actor::started(&mut actor, &mut ctx).await?;
        let actor_name = actor.name();

        let actor_loop = async move {
            // 2. configure actor loop
            while let Some(event) = rx.recv().await {
                match event {
                    ActorEvent::Exec(f) => f(&mut actor, &mut ctx).await,
                    ActorEvent::Stop(reason) => {
                        actor.stopping(&mut ctx, reason).await;
                        break;
                    }
                    ActorEvent::StopSupervisor(_err) => {}
                    ActorEvent::RemoveStream(id) => {
                        if ctx.streams.contains(id) {
                            ctx.streams.remove(id);
                        }
                    }
                }
            }

            // 3. abort streams and intervals
            ctx.abort_streams_and_intervals();

            // 4. tell actor that it stopped, and that the stram is finished
            actor.stopped(&mut ctx).await;
            actor.finished(&mut ctx).await;

            // 5. send exit signal
            tx_exit.send(()).ok();
        };

        Ok((actor_loop, addr, actor_name))
    }

    async fn stream_cycle_bound<S>(
        self,
        mut actor: A,
        mut stream: S,
    ) -> Result<(impl Future<Output = ()>, Addr<A>, Cow<'static, str>)>
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: crate::StreamHandler<S::Item>,
    {
        let Self {
            mut ctx,
            addr,
            mut rx,
            tx_exit,
        } = self;

        // 1. tell actor that it started
        Actor::started(&mut actor, &mut ctx).await?;
        let actor_name = actor.name();

        let actor_loop = async move {
            // 2. configure actor loop
            loop {
                futures::select! {
                    actor_msg = rx.recv().fuse() => {
                        match actor_msg.unwrap() {
                            ActorEvent::Exec(f) => f(&mut actor, &mut ctx).await,
                            ActorEvent::Stop(reason) => {
                                actor.stopping(&mut ctx, reason).await;
                                break;
                            }
                            ActorEvent::StopSupervisor(_err) => {}
                            ActorEvent::RemoveStream(id) => {
                                if ctx.streams.contains(id) {
                                    ctx.streams.remove(id);
                                }
                            }
                        }
                    },
                    stream_msg = stream.next().fuse() => {
                        if let Some(msg) = stream_msg {
                            actor.handle(&mut ctx, msg).await;
                        } else {
                            break;
                        }
                    },
                    complete => break,
                    default => break,
                }
            }

            // 3. abort streams and intervals
            ctx.abort_streams_and_intervals();

            // 4. tell actor that it stopped, and that the stream is finished
            actor.finished(&mut ctx).await;
            actor.stopped(&mut ctx).await;

            // 5. send exit signal
            tx_exit.send(()).ok();
        };

        Ok((actor_loop, addr, actor_name))
    }

    async fn supervised_cycle<F>(
        self,
        f: F,
    ) -> Result<(impl Future<Output = ()>, Addr<A>, Cow<'static, str>)>
    where
        F: Fn() -> A + Send + 'static,
    {
        let Self {
            mut ctx,
            addr,
            mut rx,
            tx_exit,
        } = self;

        // 0. create actor from fn
        let mut actor = f();
        // 1. tell actor that it started
        Actor::started(&mut actor, &mut ctx).await?;
        let actor_name = actor.name();

        let actor_loop = async move {
            // 2.1. configure supervisor
            'restart_loop: loop {
                // 2.2. configure actor loop
                'event_loop: loop {
                    match rx.recv().await {
                        None => break 'restart_loop,
                        Some(ActorEvent::Stop(reason)) => {
                            actor.stopping(&mut ctx, reason).await;
                            break 'event_loop;
                        }
                        Some(ActorEvent::StopSupervisor(_err)) => break 'restart_loop,
                        Some(ActorEvent::Exec(f)) => f(&mut actor, &mut ctx).await,
                        Some(ActorEvent::RemoveStream(id)) => {
                            if ctx.streams.contains(id) {
                                ctx.streams.remove(id);
                            }
                        }
                    }
                }

                // 3. abort streams and intervals
                ctx.abort_streams_and_intervals();

                // 4. tell actor that it stopped, but this is not the end
                actor.stopped(&mut ctx).await;

                // 5. recreate and restart actor
                // all addresses are still valid
                actor = f();
                actor.started(&mut ctx).await.ok();
            }

            // 3. abort streams and intervals
            ctx.abort_streams_and_intervals();

            // 4. tell actor that it stopped
            actor.stopped(&mut ctx).await;

            // 5. send exit signal
            tx_exit.send(()).ok();
        };

        Ok((actor_loop, addr, actor_name))
    }

    pub(crate) async fn start_actor(self, actor: A) -> Result<Addr<A>> {
        self.cycle(actor).await.map(Self::start)
    }

    pub(crate) async fn start_with_stream<S>(self, actor: A, stream: S) -> Result<Addr<A>>
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: crate::StreamHandler<S::Item>,
    {
        self.stream_cycle(actor, stream).await.map(Self::start)
    }

    pub(crate) async fn bind_to_stream<S>(self, actor: A, stream: S) -> Result<Addr<A>>
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: crate::StreamHandler<S::Item>,
    {
        self.stream_cycle_bound(actor, stream)
            .await
            .map(Self::start)
    }

    pub(crate) async fn start_supervised<F>(self, f: F) -> Result<Addr<A>>
    where
        F: Fn() -> A + Send + 'static,
    {
        self.supervised_cycle(f).await.map(Self::start)
    }

    pub(crate) fn start<F>((future, addr, _actor_name): (F, Addr<A>, Cow<'static, str>)) -> Addr<A>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        #[cfg(all(feature = "tracing", tokio_unstable))]
        tokio::task::Builder::new()
            .name(_actor_name.as_ref())
            .spawn(actor_loop);
        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        crate::runtime::spawn(future);
        #[cfg(all(feature = "tracing", not(tokio_unstable)))]
        compile_error!("you need to build with --rustflags tokio_unstable");
        addr
    }
}
