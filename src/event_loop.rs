use crate::{
    addr::Addr,
    channel,
    context::Context,
    error::ActorError::{SpawnError, WriteError},
    Actor, ActorResult,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{mpsc, Arc, RwLock},
};

type ExecFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub type StopReason = Option<Box<dyn std::error::Error + Send + 'static>>;

pub(crate) type ExecFn<A: Actor> =
    Box<dyn for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> ExecFuture<'a> + Send + 'static>;

pub enum Payload<A: Actor> {
    Exec(ExecFn<A>),
    Stop(StopReason),
}

impl<F, A> From<F> for Payload<A>
where
    F: FnOnce() -> ActorResult<()> + Send + Sync + 'static,
    F: for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> ExecFuture<'a> + Send + 'static,
    A: Actor,
{
    fn from(f: F) -> Self {
        Payload::Exec(Box::new(f))
    }
}


pub struct EventLoop<A: Actor> {
    ctx: Context<A>,
    addr: Addr<A>,
    rx: channel::ChanRx<A>,
    tx_exit: futures::channel::oneshot::Sender<()>,
}

impl<A: Actor> EventLoop<A> {
    pub fn start<A>(mut self, actor: A) -> ActorResult<Addr<A>>
    where
        A: Actor + Send + Sync + 'static,
    {
        let actor = Arc::new(RwLock::new(actor));
        self.spawn(actor.clone())?;

        Ok(Addr {
            ctx: Arc::new(self.ctx),
            actor,
        })
    }

    pub(crate) fn sync_loop(
        mut actor: BoxedActor,
        rx: mpsc::Receiver<Payload>,
    ) -> impl FnOnce() -> ActorResult<()> {
        move || {
            actor.started()?;
            let receiving = rx.iter();
            for payload in receiving {
                match payload {
                    Payload::Exec(exec) => exec()?,
                    Payload::Stop => break,
                }
            }
            actor.stopped()?;
            Ok(())
        }
    }

    pub fn spawn(&mut self, actor: BoxedActor) -> ActorResult<()> {
        let Some(rx) = self.ctx.take_rx() else {
            eprintln!("Cannot spawn context");
            return Err(SpawnError);
        };

        std::thread::spawn(Self::sync_loop(actor, rx));
        Ok(())
    }
}
