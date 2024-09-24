use futures::StreamExt;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
};

use crate::{event_loop::Payload as ActorEvent, error::Result};

pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<ActorEvent<A>>> + Send>>;
pub type WeakChanTx<A> = Weak<dyn TxFn<A>>;
pub type ChanTx<A> = Arc<dyn TxFn<A>>;
pub type ChanRx<A> = Box<dyn RxFn<A>>;

pub(crate) trait TxFn<A>: Send + Sync {
    fn send(&self, msg: ActorEvent<A>) -> Result<()>;
}

impl<F, A> TxFn<A> for F
where
    F: Fn(ActorEvent<A>) -> Result<()>,
    F: Send + Sync,
{
    fn send(&self, msg: ActorEvent<A>) -> Result<()> {
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

pub struct ChannelWrapper<A> {
    tx_fn: ChanTx<A>,
    rx_fn: Option<ChanRx<A>>,
}

impl<A> ChannelWrapper<A> {
    fn wrap(tx_fn: ChanTx<A>, rx_fn: ChanRx<A>) -> Self {
        ChannelWrapper {
            tx_fn,
            rx_fn: Some(rx_fn),
        }
    }
}

impl<A> ChannelWrapper<A>
where
    for<'a> A: 'a,
{
    pub fn bounded(buffer: usize) -> Self {
        ChannelWrapper::from(futures::channel::mpsc::channel::<ActorEvent<A>>(buffer))
    }

    pub fn unbounded() -> Self {
        ChannelWrapper::from(futures::channel::mpsc::unbounded::<ActorEvent<A>>())
    }
}

impl<A> ChannelWrapper<A> {
    /// Returns a strong reference to the send function
    /// This is for the [`Addr`] (and for the [`LifeCycle`] to create new [`Addr`]esses)
    pub fn tx(&self) -> ChanTx<A> {
        self.tx_fn.clone()
    }

    /// Returns a weak reference to the send function
    /// This is for the [`Context`]
    pub fn weak_tx(&self) -> WeakChanTx<A> {
        Arc::downgrade(&self.tx_fn)
    }

    /// This should be held exclusively by the [`LifeCycle`]
    pub fn rx(&mut self) -> Option<ChanRx<A>> {
        self.rx_fn.take()
    }
}

impl<A>
    From<(
        futures::channel::mpsc::UnboundedSender<ActorEvent<A>>,
        futures::channel::mpsc::UnboundedReceiver<ActorEvent<A>>,
    )> for ChannelWrapper<A>
where
    for<'a> A: 'a,
{
    fn from(
        (tx, rx): (
            futures::channel::mpsc::UnboundedSender<ActorEvent<A>>,
            futures::channel::mpsc::UnboundedReceiver<ActorEvent<A>>,
        ),
    ) -> Self {
        let send = Arc::new(move |event: ActorEvent<A>| -> Result<()> {
            let mut tx = tx.clone();
            tx.start_send(event)?;
            Ok(())
        });

        let rx = Arc::new(async_lock::Mutex::new(rx));
        let recv = Box::new(move || -> RecvFuture<A> {
            let rx = rx.clone();
            Box::pin(async move {
                let mut rx = rx.lock().await;
                rx.next().await
            })
        });
        Self::wrap(send, recv)
    }
}

impl<A>
    From<(
        futures::channel::mpsc::Sender<ActorEvent<A>>,
        futures::channel::mpsc::Receiver<ActorEvent<A>>,
    )> for ChannelWrapper<A>
where
    for<'a> A: 'a,
{
    fn from(
        (tx, rx): (
            futures::channel::mpsc::Sender<ActorEvent<A>>,
            futures::channel::mpsc::Receiver<ActorEvent<A>>,
        ),
    ) -> Self {
        let send = Arc::new(move |event: ActorEvent<A>| -> Result<()> {
            let mut wtx = tx.clone();
            wtx.start_send(event)?;
            Ok(())
        });

        let rx = Arc::new(async_lock::Mutex::new(rx));
        let recv = Box::new(move || -> RecvFuture<A> {
            let rx = rx.clone();
            Box::pin(async move {
                let mut rx = rx.lock().await;
                rx.next().await
            })
        });
        Self::wrap(send, recv)
    }
}
