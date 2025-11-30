use futures::{
    Stream as _,
    stream::{PollFn, poll_fn},
};

use std::{
    future::Future,
    pin::{Pin, pin},
    sync::{Arc, Weak},
    task,
};

use crate::{
    error::{ActorError, Result},
    event_loop::Payload,
};

trait TxFn<A>: Send + Sync {
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

trait TryTxFn<A>: Send + Sync {
    fn try_send(&self, msg: Payload<A>) -> Result<()>;
}

impl<F, A> TryTxFn<A> for F
where
    F: Fn(Payload<A>) -> Result<()>,
    F: Send + Sync,
{
    fn try_send(&self, msg: Payload<A>) -> Result<()> {
        self(msg)
    }
}

trait ForceTxFn<A>: Send + Sync {
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

type PayloadStream<A> =
    PollFn<Box<dyn FnMut(&mut task::Context<'_>) -> task::Poll<Option<Payload<A>>> + Send>>;

pub(crate) struct Tx<A> {
    tx: Arc<dyn TxFn<A>>,
    try_tx: Arc<dyn TryTxFn<A>>,
    force_tx: Arc<dyn ForceTxFn<A>>,
}

impl<A> Clone for Tx<A> {
    fn clone(&self) -> Self {
        Self {
            tx: Arc::clone(&self.tx),
            try_tx: Arc::clone(&self.try_tx),
            force_tx: Arc::clone(&self.force_tx),
        }
    }
}

impl<A> Tx<A> {
    fn new(
        tx: Arc<dyn TxFn<A>>,
        try_tx: Arc<dyn TryTxFn<A>>,
        force_tx: Arc<dyn ForceTxFn<A>>,
    ) -> Self {
        Self {
            tx,
            try_tx,
            force_tx,
        }
    }

    pub fn downgrade(&self) -> WeakTx<A> {
        WeakTx {
            tx: Arc::downgrade(&self.tx),
            try_tx: Arc::downgrade(&self.try_tx),
            force_tx: Arc::downgrade(&self.force_tx),
        }
    }

    pub fn send(&self, msg: Payload<A>) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        self.tx.send(msg)
    }

    pub fn try_send(&self, msg: Payload<A>) -> Result<()> {
        self.try_tx.try_send(msg)
    }

    pub fn force_send(&self, msg: Payload<A>) -> Result<()> {
        self.force_tx.send(msg)
    }
}

pub(crate) struct WeakTx<A> {
    tx: Weak<dyn TxFn<A>>,
    try_tx: Weak<dyn TryTxFn<A>>,
    force_tx: Weak<dyn ForceTxFn<A>>,
}

impl<A> WeakTx<A> {
    pub fn upgrade(&self) -> Option<Tx<A>> {
        Some(Tx {
            tx: self.tx.upgrade()?,
            try_tx: self.try_tx.upgrade()?,
            force_tx: self.force_tx.upgrade()?,
        })
    }
}

impl<A> Clone for WeakTx<A> {
    fn clone(&self) -> Self {
        Self {
            tx: Weak::clone(&self.tx),
            try_tx: Weak::clone(&self.try_tx),
            force_tx: Weak::clone(&self.force_tx),
        }
    }
}

pub(crate) struct Rx<A> {
    stream: PayloadStream<A>,
}

impl<A> Rx<A> {
    pub(crate) fn new(stream: PayloadStream<A>) -> Self {
        Self { stream }
    }
}

impl<A> futures::Stream for Rx<A> {
    type Item = Payload<A>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let pinned = pin!(&mut self.stream);
        pinned.poll_next(cx)
    }
}

pub(crate) struct Channel<A> {
    tx: Tx<A>,
    rx: Rx<A>,
}

impl<A> Channel<A> {
    const fn new(tx: Tx<A>, rx: Rx<A>) -> Self {
        Channel { tx, rx }
    }
}

impl<A> Channel<A>
where
    for<'a> A: 'a,
{
    pub fn bounded(buffer: usize) -> Self {
        let (tx, mut rx) = futures::channel::mpsc::channel::<Payload<A>>(buffer);
        let tx2 = tx.clone();
        let tx3 = tx.clone();

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

        let try_send = Arc::new(move |event: Payload<A>| -> Result<()> {
            let mut tx = tx3.clone();
            tx.try_send(event)
                .map_err(|_| crate::error::ActorError::AlreadyStopped)?;
            Ok(())
        });

        let force_send = Arc::new(move |event: Payload<A>| -> Result<()> {
            let mut tx = tx.clone();
            tx.start_send(event)?;
            Ok(())
        });

        let recv: PayloadStream<A> = poll_fn(Box::new(move |ctx| {
            let pinned = pin!(&mut rx);
            pinned.poll_next(ctx)
        }));

        let tx = Tx::new(send, try_send, force_send);
        let rx = Rx::new(recv);

        Self::new(tx, rx)
    }

    pub fn unbounded() -> Self {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Payload<A>>();
        let tx2 = tx.clone();
        let tx3 = tx.clone();

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

        let try_send = Arc::new(move |event: Payload<A>| -> Result<()> {
            let try_tx = tx3.clone();
            try_tx
                .unbounded_send(event)
                .map_err(|_| ActorError::AlreadyStopped)?;
            Ok(())
        });

        let force_send = Arc::new(move |event: Payload<A>| -> Result<()> {
            let mut tx_clone = tx.clone();
            tx_clone.start_send(event)?;
            Ok(())
        });

        let recv: PayloadStream<A> = poll_fn(Box::new(move |ctx| {
            let pinned = pin!(&mut rx);
            pinned.poll_next(ctx)
        }));

        let tx = Tx::new(send, try_send, force_send);
        let rx = Rx::new(recv);

        Self::new(tx, rx)
    }

    pub fn break_up(self) -> (Tx<A>, Rx<A>) {
        (self.tx, self.rx)
    }
}
