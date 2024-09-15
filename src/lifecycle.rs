use crate::{addr::ActorEvent, channel::ChannelWrapper, error::Result, Actor, Addr, Context};

use futures::FutureExt;

pub(crate) struct LifeCycle<A: Actor> {
    ctx: Context<A>,
    channel: ChannelWrapper<A>,
    tx_exit: futures::channel::oneshot::Sender<()>,
}

impl<A: Actor> LifeCycle<A> {
    pub(crate) fn new() -> Self {
        let channel = ChannelWrapper::unbounded();
        let (tx_exit, rx_exit) = futures::channel::oneshot::channel::<()>();
        let ctx = Context::new(rx_exit.shared().into(), &channel);
        Self {
            ctx,
            channel,
            tx_exit,
        }
    }

    pub(crate) fn new_bounded(buffer: usize) -> Self {
        let channel = ChannelWrapper::bounded(buffer);
        let (tx_exit, rx_exit) = futures::channel::oneshot::channel::<()>();
        let ctx = Context::new(rx_exit.shared().into(), &channel);
        Self {
            ctx,
            channel,
            tx_exit,
        }
    }
}

impl<A: Actor> LifeCycle<A>
where
    for<'a> A: 'a,
{
    /// TODO: why now implement this here in stead of in Context?
    pub fn address(&self) -> Addr<A> {
        Addr {
            actor_id: self.ctx.actor_id(),
            tx: self.channel.tx(),
            rx_exit: self.ctx.rx_exit.clone(),
        }
    }

    pub(crate) async fn start_actor(self, mut actor: A) -> Result<Addr<A>> {
        let Self {
            mut ctx,
            mut channel,
            tx_exit,
        } = self;

        let rx_exit = ctx.rx_exit.clone();
        let actor_id = ctx.actor_id();

        // Call started
        actor.started(&mut ctx).await?;

        let _actor_name = actor.name();
        let tx = channel.tx();

        let mut rx = channel.rx().unwrap();
        let actor_loop = async move {
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

            ctx.abort_streams();
            ctx.abort_intervals();

            actor.stopped(&mut ctx).await;

            tx_exit.send(()).ok();
        };

        #[cfg(all(feature = "tracing", tokio_unstable))]
        tokio::task::Builder::new()
            //.name(<A as Actor>::NAME)
            .name(_actor_name.as_ref())
            .spawn(actor_loop);
        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        crate::runtime::spawn(actor_loop);
        #[cfg(all(feature = "tracing", not(tokio_unstable)))]
        compile_error!("you need to build with --rustflags tokio_unstable");

        Ok(Addr {
            actor_id,
            tx,
            rx_exit,
        })
    }

    pub async fn start_supervised<F>(self, f: F) -> Result<Addr<A>>
    where
        F: Fn() -> A + Send + 'static,
    {
        let Self {
            mut ctx,
            mut channel,
            ..
        } = self;
        let rx_exit = ctx.rx_exit.clone();
        let actor_id = ctx.actor_id();

        // Create the actor
        let mut actor = f();

        // Call started
        actor.started(&mut ctx).await?;

        let _actor_name = actor.name();

        let tx = channel.tx();

        let mut rx = channel.rx().unwrap();
        let actor_loop = async move {
            'restart_loop: loop {
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

                ctx.abort_streams();
                ctx.abort_intervals();

                actor.stopped(&mut ctx).await;

                actor = f();
                actor.started(&mut ctx).await.ok();
            }
            actor.stopped(&mut ctx).await;
            ctx.abort_streams();
            ctx.abort_intervals();
        };

        #[cfg(all(feature = "tracing", tokio_unstable))]
        tokio::task::Builder::new()
            //.name(<A as Actor>::NAME)
            .name(_actor_name.as_ref())
            .spawn(actor_loop);
        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        crate::runtime::spawn(actor_loop);
        #[cfg(all(feature = "tracing", not(tokio_unstable)))]
        compile_error!("you need to build with --rustflags tokio_unstable");

        Ok(Addr {
            actor_id,
            tx,
            rx_exit,
        })
    }
}
