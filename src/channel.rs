#![allow(deprecated, unused_imports)]
use futures::{
    Stream as _,
    stream::{PollFn, poll_fn},
    task,
};

use std::{
    future::Future,
    marker::PhantomData,
    pin::{Pin, pin},
    sync::{Arc, Weak},
};

use crate::{Actor, Handler, Message, error::Result, event_loop::Payload};

#[deprecated(note = "use async_channel::WeakSender")]
pub type WeakChanTx<A> = Weak<dyn TxFn<A>>;

#[deprecated(note = "use async_channel::Sender")]
pub type ChanTx<A> = Arc<dyn TxFn<A>>;

// pub type MessageTx<M: Message, A: Actor + Handler<M>> = async_channel::Sender<Payload<A>>;

//todo: rename to PayloadTx and PayloadRx
pub type ActorTx<A> = async_channel::Sender<Payload<A>>;
pub type ActorRx<A> = async_channel::Receiver<Payload<A>>;

//todo: rename to WeakPayloadTx and WeakPayloadRx
pub type WeakActorTx<A> = async_channel::WeakSender<Payload<A>>;
pub type WeakActorRx<A> = async_channel::WeakReceiver<Payload<A>>;

#[deprecated(note = "use async_channel::Receiver")]
pub type PayloadStream<A> =
    PollFn<Box<dyn FnMut(&mut task::Context<'_>) -> task::Poll<Option<Payload<A>>> + Send>>;

#[deprecated(note = "use ActorTx")]
pub(crate) trait TxFn<A>: Send + Sync {
    fn send(&self, msg: Payload<A>) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;
}

impl<F, A> TxFn<A> for F
where
    F: Fn(Payload<A>) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>,
    F: Send + Sync,
{
    fn send(&self, msg: Payload<A>) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        self(msg)
    }
}

pub type ForceChanTx<A> = Arc<dyn ForceTxFn<A>>;
pub type WeakForceChanTx<A> = Weak<dyn ForceTxFn<A>>;

pub(crate) trait ForceTxFn<A>: Send + Sync {
    fn send(&self, msg: Payload<A>) -> Result<()>;
}

impl<F, A> ForceTxFn<A> for F
where
    F: Fn(Payload<A>) -> Result<()>,
    F: Send + Sync,
{
    fn send(&self, msg: Payload<A>) -> Result<()> {
        self(msg)
    }
}

// TODO: consider getting rid of this and just using `Sink` and `
pub(crate) struct Channel<A> {
    pub tx: ActorTx<A>,
    pub rx: ActorRx<A>,
}

impl<A> Channel<A> {
    fn new(tx: ActorTx<A>, rx: ActorRx<A>) -> Self {
        Channel { tx, rx }
    }
}

impl<A> Channel<A>
where
    for<'a> A: 'a,
{
    #[deprecated(note = "use async_channel::bounded directly")]
    pub fn bounded(buffer: usize) -> Self {
        let (tx, rx) = async_channel::bounded(buffer);

        Self::new(tx, rx)
    }

    #[deprecated(note = "use async_channel::unbounded directly")]
    pub fn unbounded() -> Self {
        let (tx, rx) = async_channel::unbounded();

        Self::new(tx, rx)
    }

    pub fn weak_tx(&self) -> WeakActorTx<A> {
        self.tx.downgrade()
    }
}

impl<A>
    Into<(
        async_channel::Sender<Payload<A>>,
        async_channel::Receiver<Payload<A>>,
    )> for Channel<A>
where
    for<'a> A: 'a,
{
    fn into(
        self,
    ) -> (
        async_channel::Sender<Payload<A>>,
        async_channel::Receiver<Payload<A>>,
    ) {
        (self.tx, self.rx)
    }
}
