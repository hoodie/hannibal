use std::sync::{Arc, Weak, mpsc};

use crate::Result;
use crate::payload::Payload;

pub type WeakChanTx<A> = Weak<dyn TxFn<A>>;
pub type ChanTx<A> = Arc<dyn TxFn<A>>;

pub type PayloadStream<A> = Arc<dyn StreamFn<A>>;

pub(crate) trait TxFn<A>: Send + Sync {
    fn send(&self, msg: Payload<A>) -> Result<()>;
}

impl<F, A> TxFn<A> for F
where
    F: Fn(Payload<A>) -> Result<()> + Send + Sync,
{
    fn send(&self, msg: Payload<A>) -> Result<()> {
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
    F: Fn(Payload<A>) -> Result<()> + Send + Sync,
{
    fn send(&self, msg: Payload<A>) -> Result<()> {
        self(msg)
    }
}

pub(crate) trait StreamFn<A>: Send + Sync {
    fn recv(&self) -> Option<Payload<A>>;
}

impl<A> StreamFn<A> for mpsc::Receiver<Payload<A>>
where
    mpsc::Receiver<Payload<A>>: Send + Sync + 'static,
{
    fn recv(&self) -> Option<Payload<A>> {
        self.recv().ok()
    }
}

pub(crate) struct Channel<A> {
    tx_fn: ChanTx<A>,
    force_tx_fn: ForceChanTx<A>,
    stream: PayloadStream<A>,
}

impl<A: Send + 'static> Channel<A> {
    pub fn break_up(self) -> (ForceChanTx<A>, ChanTx<A>, PayloadStream<A>) {
        (self.force_tx_fn, self.tx_fn, self.stream)
    }

    pub fn weak_force_tx(&self) -> WeakForceChanTx<A> {
        Arc::downgrade(&self.force_tx_fn)
    }
    pub fn weak_tx(&self) -> WeakChanTx<A> {
        Arc::downgrade(&self.tx_fn)
    }
}
