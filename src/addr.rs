use crate::{
    channel::ChanTx, context::RunningFuture, weak_addr::WeakAddr, Actor, ActorId, Context, Handler,
    Message, Result,
};
use futures::{channel::oneshot, Future};
use std::{
    future::ready,
    hash::{Hash, Hasher},
    pin::Pin,
    sync::Arc,
};

mod caller;
mod sender;
mod weak_caller;
mod weak_sender;

pub use self::{caller::Caller, sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender};

type ExecFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub type StopReason = Option<Box<dyn std::error::Error + Send + 'static>>;

pub(crate) type ExecFn<A> =
    Box<dyn for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> ExecFuture<'a> + Send + 'static>;

pub(crate) enum ActorEvent<A> {
    Exec(ExecFn<A>),
    Stop(StopReason),
    StopSupervisor(StopReason),
    RemoveStream(usize),
}

impl<A> ActorEvent<A> {
    pub fn exec<F>(f: F) -> ActorEvent<A>
    where
        F: for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> ExecFuture<'a> + Send + 'static,
    {
        ActorEvent::Exec(Box::new(f))
    }
}

/// The address of an actor.
///
/// When all references to [`Addr<A>`] are dropped, the actor ends.
/// You can use the [`Clone`] trait to create multiple copies of [`Addr<A>`].
pub struct Addr<A> {
    pub(crate) actor_id: ActorId,
    pub(crate) tx: ChanTx<A>,
    pub(crate) rx_exit: Option<RunningFuture>,
}

impl<A> std::fmt::Debug for Addr<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Addr")
            .field("actor_id", &self.actor_id)
            .field("rx_exit", &self.rx_exit)
            .finish_non_exhaustive()
    }
}

impl<A> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Self {
            actor_id: self.actor_id,
            tx: self.tx.clone(),
            rx_exit: self.rx_exit.clone(),
        }
    }
}

impl<A> Addr<A> {
    /// Turns an [`Addr<A>`] into a [`crate::addr::WeakAddr<A>`]
    pub fn downgrade(&self) -> WeakAddr<A> {
        WeakAddr {
            actor_id: self.actor_id,
            tx: Arc::downgrade(&self.tx),
            rx_exit: self.rx_exit.clone(),
        }
    }
}

impl<A> PartialEq for Addr<A> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id == other.actor_id
    }
}

impl<A> Hash for Addr<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.actor_id.hash(state);
    }
}

impl<A: Actor> Addr<A> {
    /// Returns the id of the actor.
    pub fn actor_id(&self) -> ActorId {
        self.actor_id
    }

    /// Stop the actor.
    pub fn stop(&mut self, err: StopReason) -> Result<()> {
        self.tx.send(ActorEvent::Stop(err))?;
        Ok(())
    }

    /// Stop the supervisor.
    ///
    /// this is ignored by normal actors
    pub fn stop_supervisor(&mut self, err: StopReason) -> Result<()> {
        self.tx.send(ActorEvent::StopSupervisor(err))?;
        Ok(())
    }

    pub fn stopped(&self) -> bool {
        self.rx_exit.as_ref().map_or(true, |x| x.peek().is_some())
    }

    /// Send a message `msg` to the actor and wait for the return value.
    pub async fn call<M: Message>(&self, msg: M) -> Result<M::Result>
    where
        A: Handler<M>,
    {
        let (repsonse_tx, response) = oneshot::channel();
        self.tx.send(ActorEvent::exec(move |actor, ctx| {
            Box::pin(async move {
                let res = Handler::handle(actor, ctx, msg).await;
                let _ = repsonse_tx.send(res);
            })
        }))?;

        Ok(response.await?)
    }

    /// Send a message `msg` to the actor without waiting for the return value.
    pub fn send<M: Message<Result = ()>>(&self, msg: M) -> Result<()>
    where
        A: Handler<M>,
    {
        self.tx.send(ActorEvent::Exec(Box::new(move |actor, ctx| {
            Box::pin(Handler::handle(actor, ctx, msg))
        })))?;
        Ok(())
    }

    fn pieces(&self) -> (ActorId, ChanTx<A>) {
        (self.actor_id, self.tx.clone())
    }

    /// Create a [`Caller<M>`] for a specific message type
    pub fn caller<M: Message>(&self) -> Caller<M>
    where
        A: Handler<M>,
    {
        Caller::from(self.to_owned())
    }

    /// Create a [`Caller<T>`] for a specific message type
    pub fn weak_caller<M: Message>(&self) -> WeakCaller<M>
    where
        A: Handler<M>,
    {
        WeakCaller::from(self.pieces())
    }

    /// Create a [`Sender<M>`] for a specific message type
    pub fn sender<M: Message<Result = ()>>(&self) -> Sender<M>
    where
        A: Handler<M>,
    {
        Sender::from(self.pieces())
    }

    /// Create a [`WeakSender<T>`] for a specific message type
    pub fn weak_sender<M: Message<Result = ()>>(&self) -> WeakSender<M>
    where
        A: Handler<M>,
    {
        WeakSender::from(self.pieces())
    }

    /// Wait for an actor to finish, and if the actor has finished, the function returns immediately.
    pub async fn wait_for_stop(self) {
        if let Some(rx_exit) = self.rx_exit {
            rx_exit.await.ok();
        } else {
            ready(()).await;
        }
    }
}
