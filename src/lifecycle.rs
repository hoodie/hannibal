use crate::{addr::ActorEvent, error::Result, Actor, Addr, Context};

use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    FutureExt, StreamExt,
};

pub(crate) struct LifeCycle<A: Actor> {
    ctx: Context<A>,
    tx: std::sync::Arc<UnboundedSender<ActorEvent<A>>>,
    rx: UnboundedReceiver<ActorEvent<A>>,
    tx_exit: futures::channel::oneshot::Sender<()>,
}

impl<A: Actor> LifeCycle<A> {
    pub(crate) fn new() -> Self {
        // let (tx_exit, rx_exit) = tokio::sync::watch::channel(Running);
        let (tx_exit, rx_exit) = futures::channel::oneshot::channel::<()>();
        let running = rx_exit.shared();
        let (ctx, rx, tx) = Context::new(Some(running));
        Self {
            ctx,
            tx,
            rx,
            tx_exit,
        }
    }

    pub(crate) fn address(&self) -> Addr<A> {
        self.ctx.address()
    }

    pub(crate) async fn start_actor(self, mut actor: A) -> Result<Addr<A>> {
        let Self {
            mut ctx,
            mut rx,
            tx,
            tx_exit,
        } = self;

        let rx_exit = ctx.rx_exit.clone();
        let actor_id = ctx.actor_id();

        // Call started
        actor.started(&mut ctx).await?;

        let _actor_name = actor.name();

        let actor_loop = async move {
            while let Some(event) = rx.next().await {
                match event {
                    ActorEvent::Exec(f) => f(&mut actor, &mut ctx).await,
                    ActorEvent::Stop(_err) => break,
                    ActorEvent::StopSupervisor(_err) => {}
                    ActorEvent::RemoveStream(id) => {
                        if ctx.streams.contains(id) {
                            ctx.streams.remove(id);
                        }
                    }
                }
            }

            actor.stopped(&mut ctx).await;

            ctx.abort_streams();
            ctx.abort_intervals();

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
            mut rx,
            tx,
            ..
        } = self;

        let addr = Addr {
            actor_id: ctx.actor_id(),
            tx,
            rx_exit: ctx.rx_exit.clone(),
        };

        // Create the actor
        let mut actor = f();

        // Call started
        actor.started(&mut ctx).await?;

        let _actor_name = actor.name();

        let actor_loop = async move {
            'restart_loop: loop {
                'event_loop: loop {
                    match rx.next().await {
                        None => break 'restart_loop,
                        Some(ActorEvent::Stop(_err)) => break 'event_loop,
                        Some(ActorEvent::StopSupervisor(_err)) => break 'restart_loop,
                        Some(ActorEvent::Exec(f)) => f(&mut actor, &mut ctx).await,
                        Some(ActorEvent::RemoveStream(id)) => {
                            if ctx.streams.contains(id) {
                                ctx.streams.remove(id);
                            }
                        }
                    }
                }

                actor.stopped(&mut ctx).await;
                ctx.abort_streams();
                ctx.abort_intervals();

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

        Ok(addr)
    }
}
