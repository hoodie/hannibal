use crate::error::bail;
use crate::{Actor, ActorId, Caller, Context, Error, Handler, Message, Result, Sender};
use futures::channel::{mpsc, oneshot};
use futures::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::{Arc, Weak};

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
    pub(crate) tx: Arc<mpsc::UnboundedSender<ActorEvent<A>>>,
    pub(crate) rx_exit: Option<async_broadcast::Receiver<()>>,
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
    /// Turns an [`Addr<A>`] into a [`WeakAddr<A>`]
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
        mpsc::UnboundedSender::clone(&*self.tx).start_send(ActorEvent::Stop(err))?;
        Ok(())
    }

    /// Stop the supervisor.
    ///
    /// this is ignored by normal actors
    pub fn stop_supervisor(&mut self, err: Option<Error>) -> Result<()> {
        mpsc::UnboundedSender::clone(&*self.tx).start_send(ActorEvent::StopSupervisor(err))?;
        Ok(())
    }

    pub fn stopped(&self) -> bool {
        self.rx_exit.as_ref().map_or(true, |x| x.is_closed())
    }

    /// Send a message `msg` to the actor and wait for the return value.
    pub async fn call<T: Message>(&self, msg: T) -> Result<T::Result>
    where
        A: Handler<T>,
    {
        let (tx, rx) = oneshot::channel();
        mpsc::UnboundedSender::clone(&*self.tx).start_send(ActorEvent::Exec(Box::new(
            move |actor, ctx| {
                Box::pin(async move {
                    let res = Handler::handle(actor, ctx, msg).await;
                    let _ = tx.send(res);
                })
            },
        )))?;

        Ok(rx.await?)
    }

    /// Send a message `msg` to the actor without waiting for the return value.
    pub fn send<T: Message<Result = ()>>(&self, msg: T) -> Result<()>
    where
        A: Handler<T>,
    {
        mpsc::UnboundedSender::clone(&*self.tx).start_send(ActorEvent::Exec(Box::new(
            move |actor, ctx| {
                Box::pin(async move {
                    Handler::handle(actor, ctx, msg).await;
                })
            },
        )))?;
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

                        mpsc::UnboundedSender::clone(&tx).start_send(ActorEvent::Exec(
                            Box::new(move |actor, ctx| {
                                Box::pin(async move {
                                    let res = Handler::handle(&mut *actor, ctx, msg).await;
                                    let _ = oneshot_tx.send(res);
                                })
                            }),
                        ))?;
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
    pub fn sender<T: Message<Result = ()>>(&self) -> Sender<T>
    where
        A: Handler<T>,
    {
        let weak_tx = Arc::downgrade(&self.tx);
        let sender_fn = Box::new(move |msg| match weak_tx.upgrade() {
            Some(tx) => {
                mpsc::UnboundedSender::clone(&tx).start_send(ActorEvent::Exec(Box::new(
                    move |actor, ctx| {
                        Box::pin(async move {
                            Handler::handle(&mut *actor, ctx, msg).await;
                        })
                    },
                )))?;
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
        if let Some(mut rx_exit) = self.rx_exit {
            rx_exit.recv().await.ok();
        } else {
            // futures::future::pending::<()>().await;
            std::future::ready(()).await;
        }
    }
}

/// Weak version of [`Addr<A>`].
///
/// This address will not prolong the lifetime of the actor.
/// In order to use a [`WeakAddr<A>`] you need to "upgrade" it to a proper [`Addr<A>`].
pub struct WeakAddr<A> {
    pub(crate) actor_id: ActorId,
    pub(crate) tx: Weak<mpsc::UnboundedSender<ActorEvent<A>>>,
    pub(crate) rx_exit: Option<async_broadcast::Receiver<()>>,
}

impl<A> std::fmt::Debug for WeakAddr<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeakAddr")
            .field("actor_id", &self.actor_id)
            // .field("tx", &self.tx)
            .field("rx_exit", &self.rx_exit)
            .finish()
    }
}

impl<A> PartialEq for WeakAddr<A> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id == other.actor_id
    }
}

impl<A> Hash for WeakAddr<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.actor_id.hash(state);
    }
}

impl<A> WeakAddr<A> {
    /// Attempts to turn a [`WeakAddr<A>`] into an [`Addr<A>`].
    ///
    /// If the original [`Addr<A>`] has already been dropped this method will return [`None`]
    pub fn upgrade(&self) -> Option<Addr<A>> {
        self.tx.upgrade().map(|tx| Addr {
            actor_id: self.actor_id,
            tx,
            rx_exit: self.rx_exit.clone(),
        })
    }

    /// Try to upgrade to [`Addr`] and call [`Addr::send`]
    pub fn upgrade_send<T: Message<Result = ()>>(&self, msg: T) -> Result<()>
    where
        A: Handler<T>,
    {
        if let Some(addr) = self.upgrade() {
            addr.send(msg)
        } else {
            bail!("cannot upgrade");
        }
    }
}

impl<A> Clone for WeakAddr<A> {
    fn clone(&self) -> Self {
        Self {
            actor_id: self.actor_id,
            tx: self.tx.clone(),
            rx_exit: self.rx_exit.clone(),
        }
    }
}

impl<A: Actor> WeakAddr<A> {
    /// Returns the id of the actor.
    pub fn actor_id(&self) -> ActorId {
        self.actor_id
    }
}
