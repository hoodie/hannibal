use crate::{
    addr::{ActorEvent, StopReason},
    broker::{Subscribe, Unsubscribe},
    channel::{ChannelWrapper, WeakChanTx},
    runtime::{sleep, spawn},
    Actor, ActorId, Addr, Broker, Handler, Message, Result, Service, StreamHandler,
};
use futures::{
    future::{AbortHandle, Abortable},
    Stream, StreamExt,
};
use once_cell::sync::OnceCell;
use slab::Slab;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

pub type RunningFuture = futures::future::Shared<futures::channel::oneshot::Receiver<()>>;

/// An actor execution context.
pub struct Context<A> {
    actor_id: ActorId,
    weak_tx: WeakChanTx<A>,
    pub(crate) rx_exit: Option<RunningFuture>,
    pub(crate) streams: Slab<AbortHandle>,
    pub(crate) intervals: Slab<AbortHandle>,
}

impl<A: Actor> Context<A>
where
    for<'a> A: 'a,
{
    fn next_id() -> u64 {
        static ACTOR_ID: OnceCell<AtomicU64> = OnceCell::new();
        ACTOR_ID
            .get_or_init(Default::default)
            .fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn new(rx_exit: Option<RunningFuture>, channel: &ChannelWrapper<A>) -> Self {
        let actor_id = Self::next_id();

        Self {
            actor_id,
            weak_tx: channel.weak_tx(),
            rx_exit,
            streams: Default::default(),
            intervals: Default::default(),
        }
    }

    /// Returns the address of the actor.
    pub fn address(&self) -> Addr<A> {
        Addr {
            actor_id: self.actor_id,
            // This getting unwrap panics
            tx: self.weak_tx.upgrade().unwrap(),
            rx_exit: self.rx_exit.clone(),
        }
    }

    /// Returns the id of the actor.
    pub fn actor_id(&self) -> ActorId {
        self.actor_id
    }

    /// Stop the actor.
    pub fn stop(&self, err: StopReason) {
        if let Some(tx) = self.weak_tx.upgrade() {
            tx.send(ActorEvent::Stop(err)).ok();
        }
    }

    /// Stop the supervisor.
    ///
    /// this is ignored by normal actors
    pub fn stop_supervisor(&self, err: StopReason) {
        if let Some(tx) = self.weak_tx.upgrade() {
            tx.send(ActorEvent::StopSupervisor(err)).ok();
        }
    }

    pub fn stopped(&self) -> bool {
        self.rx_exit.as_ref().map_or(true, |x| x.peek().is_some())
    }

    pub fn abort_intervals(&mut self) {
        for handle in self.intervals.drain() {
            handle.abort()
        }
    }

    pub fn abort_streams(&mut self) {
        for handle in self.streams.drain() {
            handle.abort();
        }
    }

    /// Create a stream handler for the actor.
    ///
    /// # Examples
    /// ```rust
    /// use hannibal::*;
    /// use futures::stream;
    /// use std::time::Duration;
    ///
    /// #[message(result = i32)]
    /// struct GetSum;
    ///
    /// #[derive(Default)]
    /// struct MyActor(i32);
    ///
    /// impl StreamHandler<i32> for MyActor {
    ///     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
    ///         self.0 += msg;
    ///     }
    ///
    ///     async fn started(&mut self, _ctx: &mut Context<Self>) {
    ///         println!("stream started");
    ///     }
    ///
    ///     async fn finished(&mut self, _ctx: &mut Context<Self>) {
    ///         println!("stream finished");
    ///     }
    /// }
    ///
    /// impl Handler<GetSum> for MyActor {
    ///     async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: GetSum) -> i32 {
    ///         self.0
    ///     }
    /// }
    ///
    /// impl Actor for MyActor {
    ///     async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()> {
    ///         let values = (0..100).collect::<Vec<_>>();
    ///         ctx.add_stream(stream::iter(values));
    ///         Ok(())
    ///     }
    /// }
    ///
    /// #[hannibal::main]
    /// async fn main() -> Result<()> {
    ///     let mut addr = MyActor::start_default().await?;
    ///     sleep(Duration::from_secs(1)).await; // Wait for the stream to complete
    ///     let res = addr.call(GetSum).await?;
    ///     assert_eq!(res, (0..100).sum::<i32>());
    ///     Ok(())
    /// }
    /// ```
    pub fn add_stream<S>(&mut self, mut stream: S)
    where
        S: Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: StreamHandler<S::Item>,
    {
        let weak_tx = self.weak_tx.clone();
        let entry = self.streams.vacant_entry();
        let id = entry.key();
        let (handle, registration) = futures::future::AbortHandle::new_pair();
        entry.insert(handle);

        let fut = async move {
            if let Some(tx) = weak_tx.upgrade() {
                tx.send(ActorEvent::exec(move |actor, ctx| {
                    Box::pin(StreamHandler::started(actor, ctx))
                }))
                .ok(); // TODO: don't eat error
            } else {
                return;
            }

            while let Some(msg) = stream.next().await {
                if let Some(tx) = weak_tx.upgrade() {
                    let res = tx.send(ActorEvent::exec(move |actor, ctx| {
                        Box::pin(StreamHandler::handle(actor, ctx, msg))
                    }));
                    if res.is_err() {
                        return;
                    }
                } else {
                    return;
                }
            }

            if let Some(tx) = weak_tx.upgrade() {
                tx.send(ActorEvent::exec(move |actor, ctx| {
                    Box::pin(StreamHandler::finished(actor, ctx))
                }))
                .ok(); // TODO: don't eat error
            }

            if let Some(tx) = weak_tx.upgrade() {
                tx.send(ActorEvent::RemoveStream(id)).ok();
            }
        };
        spawn(Abortable::new(fut, registration));
    }

    /// Sends the message `msg` to self after a specified period of time.
    ///
    /// We use `Sender` instead of `Addr` so that the interval doesn't keep reference to address and prevent the actor from being dropped and stopped

    pub fn send_later<T>(&mut self, msg: T, after: Duration)
    where
        A: Handler<T>,
        T: Message<Result = ()>,
    {
        let sender = self.address().sender();
        let entry = self.intervals.vacant_entry();
        let (handle, registration) = futures::future::AbortHandle::new_pair();
        entry.insert(handle);

        spawn(Abortable::new(
            async move {
                sleep(after).await;
                sender.send(msg).ok();
            },
            registration,
        ));
    }

    /// Sends the message  to self, at a specified fixed interval.
    /// The message is created each time using a closure `f`.
    pub fn send_interval_with<T, F>(&mut self, f: F, dur: Duration)
    where
        A: Handler<T>,
        F: Fn() -> T + Sync + Send + 'static,
        T: Message<Result = ()>,
    {
        let sender = self.address().sender();

        let entry = self.intervals.vacant_entry();
        let (handle, registration) = futures::future::AbortHandle::new_pair();
        entry.insert(handle);

        spawn(Abortable::new(
            async move {
                loop {
                    sleep(dur).await;
                    if sender.send(f()).is_err() {
                        break;
                    }
                }
            },
            registration,
        ));
    }

    /// Sends the message `msg` to self, at a specified fixed interval.
    pub fn send_interval<T>(&mut self, msg: T, dur: Duration)
    where
        A: Handler<T>,
        T: Message<Result = ()> + Clone + Sync,
    {
        self.send_interval_with(move || msg.clone(), dur);
    }

    /// Subscribes to a message of a specified type.
    pub async fn subscribe<T: Message<Result = ()>>(&self) -> Result<()>
    where
        A: Handler<T>,
    {
        let broker = Broker::<T>::from_registry().await?;
        let sender = self.address().sender();
        broker
            .send(Subscribe {
                id: self.actor_id,
                sender,
            })
            .ok();
        Ok(())
    }

    /// Unsubscribe to a message of a specified type.
    pub async fn unsubscribe<T: Message<Result = ()>>(&self) -> Result<()> {
        let broker = Broker::<T>::from_registry().await?;
        broker.send(Unsubscribe { id: self.actor_id })
    }
}
