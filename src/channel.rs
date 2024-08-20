use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
};

use futures::{SinkExt, StreamExt};

use crate::event_loop::Payload;

type Result<T> = std::result::Result<T, ()>;

pub type RecvFuture = Pin<Box<dyn Future<Output = Option<Payload>> + Send>>;
pub type WeakChanTx = Weak<dyn TxFn>;
pub type ChanTx = Arc<dyn TxFn>;
pub type ChanRx = Box<dyn RxFn>;

pub(crate) trait TxFn: Send + Sync {
    fn send(&self, msg: Payload) -> Result<()>;
}

impl<F> TxFn for F
where
    F: Fn(Payload) -> Result<()>,
    F: Send + Sync,
{
    fn send(&self, msg: Payload) -> Result<()> {
        self(msg)
    }
}

pub(crate) trait RxFn: Send + Sync {
    fn recv(&mut self) -> RecvFuture;
}

impl<F> RxFn for F
where
    F: FnMut() -> RecvFuture,
    F: Send + Sync,
{
    fn recv(&mut self) -> RecvFuture {
        self()
    }
}

pub struct Channel {
    tx_fn: ChanTx,
    rx_fn: Option<ChanRx>,
}

impl Channel {
    pub fn bounded(buffer: usize) -> Self {
        Channel::from(futures::channel::mpsc::channel::<Payload>(buffer))
    }

    // pub fn unbounded() -> Self {
    //     Channel::from(futures_channel::futures_channel::mpsc::unbounded::<Payload>())
    // }

    /// Returns a strong reference to the send function
    /// This is for the [`Addr`] (and for the [`EventLoop`] to create new [`Addr`]esses)
    pub fn tx(&self) -> ChanTx {
        self.tx_fn.clone()
    }

    /// Returns a weak reference to the send function
    /// This is for the [`Context`]
    pub fn weak_tx(&self) -> WeakChanTx {
        Arc::downgrade(&self.tx_fn)
    }

    /// This should be held exclusively by the [`EventLoop`]
    pub fn rx(&mut self) -> Option<ChanRx> {
        self.rx_fn.take()
    }

    fn wrap(tx_fn: ChanTx, rx_fn: ChanRx) -> Self {
        Channel {
            tx_fn,
            rx_fn: Some(rx_fn),
        }
    }
}

impl
    From<(
        futures::channel::mpsc::Sender<Payload>,
        futures::channel::mpsc::Receiver<Payload>,
    )> for Channel
{
    fn from(
        (tx, rx): (
            futures::channel::mpsc::Sender<Payload>,
            futures::channel::mpsc::Receiver<Payload>,
        ),
    ) -> Self {
        let stx = tx.clone();
        let _send_async = Arc::new(
            move |event: Payload| -> Pin<Box<dyn Future<Output = Result<()>>>> {
                let mut tx = stx.clone();
                Box::pin(async move {
                    // let ftx: dyn Sink<Payload> = &tx;
                    // SinkExt::send_all(&mut tx, event);
                    tx.send(event).await.unwrap(); //.map_err(|_| {}).await?;
                    Ok(())
                })
            },
        );

        let send = Arc::new(move |event: Payload| -> Result<()> {
            let mut tx = tx.clone();
            tx.start_send(event).map_err(|_| {})?;
            Ok(())
        });

        let rx = Arc::new(async_lock::Mutex::new(rx));
        let recv = Box::new(move || {
            let rx = rx.clone();
            Box::pin(async move {
                let mut rx = rx.lock().await;
                rx.next().await
            }) as RecvFuture
        });
        Self::wrap(send, recv)
    }
}
