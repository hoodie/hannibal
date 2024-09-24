use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    StreamExt,
};

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
};

use crate::{addr::Payload, error::Result};

pub type RecvFuture<A> = Pin<Box<dyn Future<Output = Option<Payload<A>>> + Send>>;
pub type WeakChanTx<A> = Weak<dyn TxFn<A>>;
pub type ChanTx<A> = Arc<dyn TxFn<A>>;
pub type ChanRx<A> = Box<dyn RxFn<A>>;

pub(crate) trait TxFn<A>: Send + Sync {
    fn send(&self, msg: Payload<A>) -> Result<()>;
}

impl<F, A> TxFn<A> for F
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

pub(crate) struct ChannelWrapper<A> {
    tx_fn: ChanTx<A>,
    rx_fn: ChanRx<A>,
}

impl<A> ChannelWrapper<A> {
    fn wrap(tx_fn: ChanTx<A>, rx_fn: ChanRx<A>) -> Self {
        ChannelWrapper { tx_fn, rx_fn }
    }
}

impl<A> ChannelWrapper<A>
where
    for<'a> A: 'a,
{
    pub fn unbounded() -> Self {
        ChannelWrapper::from(futures::channel::mpsc::unbounded::<Payload<A>>())
    }

    pub fn break_up(self) -> (ChanTx<A>, ChanRx<A>) {
        (self.tx_fn, self.rx_fn)
    }
}

impl<A> ChannelWrapper<A> {
    pub fn weak_tx(&self) -> WeakChanTx<A> {
        Arc::downgrade(&self.tx_fn)
    }
}

impl<A> From<(UnboundedSender<Payload<A>>, UnboundedReceiver<Payload<A>>)> for ChannelWrapper<A>
where
    for<'a> A: 'a,
{
    fn from((tx, rx): (UnboundedSender<Payload<A>>, UnboundedReceiver<Payload<A>>)) -> Self {
        let send = Arc::new(move |event: Payload<A>| -> Result<()> {
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
