use futures::StreamExt;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
};

use crate::{environment::Payload, error::Result};

pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<Payload<A>>> + Send>>;
pub type WeakChanTx<A> = Weak<dyn TxFn<A>>;
pub type ChanTx<A> = Arc<dyn TxFn<A>>;
pub type ChanRx<A> = Box<dyn RxFn<A>>;

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

pub(crate) trait RxFn<A>: Send + Sync {
    fn recv(&mut self) -> RecvFuture<A>;
}

impl<F, A> RxFn<A> for F
where
    F: FnMut() -> RecvFuture<A>,
    F: Send + Sync,
{
    fn recv(&mut self) -> RecvFuture<A> {
        self()
    }
}

// TODO: consider getting rid of this and just using `Sink` and `
pub(crate) struct Channel<A> {
    tx_fn: ChanTx<A>,
    force_tx_fn: ForceChanTx<A>,
    rx_fn: ChanRx<A>,
}

impl<A> Channel<A> {
    fn new(tx_fn: ChanTx<A>, force_tx_fn: ForceChanTx<A>, rx_fn: ChanRx<A>) -> Self {
        Channel {
            tx_fn,
            force_tx_fn,
            rx_fn,
        }
    }
}

impl<A> Channel<A>
where
    for<'a> A: 'a,
{
    pub fn bounded(buffer: usize) -> Self {
        let (tx, rx) = futures::channel::mpsc::channel::<Payload<A>>(buffer);
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
            tx.start_send(event)?;
            Ok(())
        });

        let rx = Arc::new(async_lock::Mutex::new(rx));
        let recv = Box::new(move || -> RecvFuture<A> {
            let rx = Arc::clone(&rx);
            Box::pin(async move {
                let mut rx = rx.lock().await;
                rx.next().await
            })
        });
        Self::new(send, force_send, recv)
    }

    pub fn unbounded() -> Self {
        let (tx, rx) = futures::channel::mpsc::unbounded::<Payload<A>>();
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
            tx.start_send(event)?;
            Ok(())
        });

        let rx = Arc::new(async_lock::Mutex::new(rx));
        let recv = Box::new(move || -> RecvFuture<A> {
            let rx = Arc::clone(&rx);
            Box::pin(async move {
                let mut rx = rx.lock().await;
                rx.next().await
            })
        });
        Self::new(send, force_send, recv)
    }

    pub fn break_up(self) -> (ForceChanTx<A>, ChanTx<A>, ChanRx<A>) {
        (self.force_tx_fn, self.tx_fn, self.rx_fn)
    }

    pub fn weak_force_tx(&self) -> WeakForceChanTx<A> {
        Arc::downgrade(&self.force_tx_fn)
    }
    pub fn weak_tx(&self) -> WeakChanTx<A> {
        Arc::downgrade(&self.tx_fn)
    }
}
