use crate::{
    channel::ChanTx, context::RunningFuture, weak_addr::WeakAddr, Actor, ActorId, Context, Error,
    Handler, Message, Result,
};
use futures::{channel::oneshot, Future};
use std::{
    hash::{Hash, Hasher},
    pin::Pin,
    sync::Arc,
};

pub(crate) mod caller;
pub(crate) mod sender;
pub(crate) mod tester;
pub use self::{caller::Caller, sender::Sender};

type ExecFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub(crate) type ExecFn<A> =
    Box<dyn for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> ExecFuture<'a> + Send + 'static>;

pub(crate) enum ActorEvent<A> {
    Exec(ExecFn<A>),
    Stop(Option<Error>),
    StopSupervisor(Option<Error>),
    RemoveStream(usize),
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
    pub fn stop(&mut self, err: Option<Error>) -> Result<()> {
        self.tx.send(ActorEvent::Stop(err))?;
        Ok(())
    }

    /// Stop the supervisor.
    ///
    /// this is ignored by normal actors
    pub fn stop_supervisor(&mut self, err: Option<Error>) -> Result<()> {
        self.tx.send(ActorEvent::StopSupervisor(err))?;
        Ok(())
    }

    pub fn stopped(&self) -> bool {
        self.rx_exit.as_ref().map_or(true, |x| x.peek().is_some())
    }

    /// Send a message `msg` to the actor and wait for the return value.
    pub async fn call<T: Message>(&self, msg: T) -> Result<T::Result>
    where
        A: Handler<T>,
    {
        let (tx, rx) = oneshot::channel();
        self.tx.send(ActorEvent::Exec(Box::new(move |actor, ctx| {
            Box::pin(async move {
                let res = Handler::handle(actor, ctx, msg).await;
                let _ = tx.send(res);
            })
        })))?;

        Ok(rx.await?)
    }

    /// Send a message `msg` to the actor without waiting for the return value.
    pub fn send<T: Message<Result = ()>>(&self, msg: T) -> Result<()>
    where
        A: Handler<T>,
    {
        self.tx.send(ActorEvent::Exec(Box::new(move |actor, ctx| {
            Box::pin(async move {
                Handler::handle(actor, ctx, msg).await;
            })
        })))?;
        Ok(())
    }

    /// Create a [`Caller<T>`] for a specific message type
    pub fn caller<T: Message>(&self) -> Caller<T>
    where
        A: Handler<T>,
    {
        let weak_tx = Arc::downgrade(&self.tx);
        let caller_fn = Box::new(move |msg| {
            let weak_tx_option = weak_tx.upgrade();
            Box::pin(async move {
                match weak_tx_option {
                    Some(tx) => {
                        let (oneshot_tx, oneshot_rx) = oneshot::channel();

                        tx.send(ActorEvent::Exec(Box::new(move |actor, ctx| {
                            Box::pin(async move {
                                let res = Handler::handle(&mut *actor, ctx, msg).await;
                                let _ = oneshot_tx.send(res);
                            })
                        })))?;
                        Ok(oneshot_rx.await?)
                    }
                    None => Err(crate::error::anyhow!("Actor Dropped")),
                }
            }) as Pin<Box<dyn Future<Output = Result<T::Result>>>>
        });

        let weak_tx = Arc::downgrade(&self.tx);
        let test_fn = Box::new(move || weak_tx.strong_count() > 0);

        Caller {
            actor_id: self.actor_id,
            caller_fn,
            test_fn,
        }
    }

    /// Create a [`Sender<T>`] for a specific message type
    pub fn strong_sender<T: Message<Result = ()>>(&self) -> Sender<T>
    where
        A: Handler<T>,
    {
        let tx = Arc::clone(&self.tx);
        let sender_fn = Box::new(move |msg| {
            tx.send(ActorEvent::Exec(Box::new(move |actor, ctx| {
                Box::pin(async move {
                    Handler::handle(&mut *actor, ctx, msg).await;
                })
            })))?;
            Ok(())
        });

        let weak_tx = Arc::downgrade(&self.tx);
        let test_fn = Box::new(move || weak_tx.strong_count() > 0);

        Sender {
            actor_id: self.actor_id,
            sender_fn,
            test_fn,
        }
    }

    /// Create a [`Sender<T>`] for a specific message type
    pub fn sender<T: Message<Result = ()>>(&self) -> Sender<T>
    where
        A: Handler<T>,
    {
        let weak_tx = Arc::downgrade(&self.tx);
        let sender_fn = Box::new(move |msg| match weak_tx.upgrade() {
            Some(tx) => {
                tx.send(ActorEvent::Exec(Box::new(move |actor, ctx| {
                    Box::pin(async move {
                        Handler::handle(&mut *actor, ctx, msg).await;
                    })
                })))?;
                Ok(())
            }
            None => Err(crate::error::anyhow!("Actor Dropped")),
        });

        let weak_tx = Arc::downgrade(&self.tx);
        let test_fn = Box::new(move || weak_tx.strong_count() > 0);

        Sender {
            actor_id: self.actor_id,
            sender_fn,
            test_fn,
        }
    }

    /// Wait for an actor to finish, and if the actor has finished, the function returns immediately.
    pub async fn wait_for_stop(self) {
        if let Some(rx_exit) = self.rx_exit {
            rx_exit.await.ok();
        } else {
            std::future::ready(()).await;
        }
    }
}
