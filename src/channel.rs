use futures::{
    Stream as _,
    stream::{PollFn, poll_fn},
    task,
};

use std::{
    future::Future,
    pin::{Pin, pin},
    sync::{Arc, Weak},
};

use crate::{environment::Payload, error::Result};

pub type WeakChanTx<A> = Weak<dyn TxFn<A>>;
pub type ChanTx<A> = Arc<dyn TxFn<A>>;

pub type PayloadStream<A> =
    PollFn<Box<dyn FnMut(&mut task::Context<'_>) -> task::Poll<Option<Payload<A>>> + Send>>;

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
    tx_fn: ChanTx<A>,
    force_tx_fn: ForceChanTx<A>,
    stream: PayloadStream<A>,
}

impl<A> Channel<A> {
    fn new(tx_fn: ChanTx<A>, force_tx_fn: ForceChanTx<A>, stream: PayloadStream<A>) -> Self {
        Channel {
            tx_fn,
            force_tx_fn,
            stream,
        }
    }
}

impl<A> Channel<A>
where
    for<'a> A: 'a,
{
    pub fn bounded(buffer: usize) -> Self {
        let (tx, mut rx) = futures::channel::mpsc::channel::<Payload<A>>(buffer);
        let tx2 = tx.clone();

        let send = Arc::new(
            move |event: Payload<A>| -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
                let tx = tx2.clone();
                Box::pin(async move {
                    let mut tx = tx.clone();
                    futures::SinkExt::send(&mut tx, event).await?;
                    Ok(())
                })
            },
        );

        let force_send = Arc::new(move |event: Payload<A>| -> Result<()> {
            let mut tx = tx.clone();
            // THIS IS A BUG!
            // Just calling this without checking for readyness will just queue this and ignore the bound
            tx.start_send(event)?;
            Ok(())
        });

        let recv: PayloadStream<A> = poll_fn(Box::new(move |ctx| {
            let pinned = pin!(&mut rx);
            pinned.poll_next(ctx)
        }));

        Self::new(send, force_send, recv)
    }

    pub fn unbounded() -> Self {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Payload<A>>();
        let tx2 = tx.clone();

        let send = Arc::new(
            move |event: Payload<A>| -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
                let tx = tx2.clone();
                Box::pin(async move {
                    let mut tx = tx.clone();
                    // THIS IS A BUG!
                    // Just calling this without checking for readyness will just queue this and ignore the bound
                    futures::SinkExt::send(&mut tx, event).await?;
                    Ok(())
                })
            },
        );

        let force_send = Arc::new(move |event: Payload<A>| -> Result<()> {
            log::trace!("sending (unbounded {})", tx.len());
            let mut tx = tx.clone();
            tx.start_send(event)?;
            Ok(())
        });
        let recv: PayloadStream<A> = poll_fn(Box::new(move |ctx| {
            let pinned = pin!(&mut rx);
            pinned.poll_next(ctx)
        }));

        Self::new(send, force_send, recv)
    }

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
