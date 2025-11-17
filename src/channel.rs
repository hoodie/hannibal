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

use crate::{error::Result, event_loop::Payload};

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

pub(crate) struct Sender<A> {
    tx: Arc<dyn TxFn<A>>,
    force_tx: Arc<dyn ForceTxFn<A>>,
}

impl<A> Clone for Sender<A> {
    fn clone(&self) -> Self {
        Self {
            tx: Arc::clone(&self.tx),
            force_tx: Arc::clone(&self.force_tx),
        }
    }
}

impl<A> Sender<A> {
    fn new(tx: Arc<dyn TxFn<A>>, force_tx: Arc<dyn ForceTxFn<A>>) -> Self {
        Self { tx, force_tx }
    }

    pub fn downgrade(&self) -> WeakSender<A> {
        WeakSender {
            tx: Arc::downgrade(&self.tx),
            force_tx: Arc::downgrade(&self.force_tx),
        }
    }

    pub fn send(&self, msg: Payload<A>) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        self.tx.send(msg)
    }

    pub fn force_send(&self, msg: Payload<A>) -> Result<()> {
        self.force_tx.send(msg)
    }
}

pub(crate) struct WeakSender<A> {
    tx: Weak<dyn TxFn<A>>,
    force_tx: Weak<dyn ForceTxFn<A>>,
}

impl<A> WeakSender<A> {
    pub fn upgrade(&self) -> Option<Sender<A>> {
        Some(Sender {
            tx: self.tx.upgrade()?,
            force_tx: self.force_tx.upgrade()?,
        })
    }
}

impl<A> Clone for WeakSender<A> {
    fn clone(&self) -> Self {
        Self {
            tx: Weak::clone(&self.tx),
            force_tx: Weak::clone(&self.force_tx),
        }
    }
}

pub(crate) struct Receiver<A> {
    stream: PayloadStream<A>,
}

impl<A> Receiver<A> {
    pub(crate) fn new(stream: PayloadStream<A>) -> Self {
        Self { stream }
    }
}

impl<A> futures::Stream for Receiver<A> {
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
    sender: Sender<A>,
    receiver: Receiver<A>,
}

impl<A> Channel<A> {
    const fn new(sender: Sender<A>, receiver: Receiver<A>) -> Self {
        Channel { sender, receiver }
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
            tx.start_send(event)?;
            Ok(())
        });

        let recv: PayloadStream<A> = poll_fn(Box::new(move |ctx| {
            let pinned = pin!(&mut rx);
            pinned.poll_next(ctx)
        }));

        let sender = Sender::new(send, force_send);
        let receiver = Receiver::new(recv);

        Self::new(sender, receiver)
    }

    pub fn unbounded() -> Self {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Payload<A>>();
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
            log::trace!("sending (unbounded {})", tx.len());
            let mut tx = tx.clone();
            tx.start_send(event)?;
            Ok(())
        });

        let recv: PayloadStream<A> = poll_fn(Box::new(move |ctx| {
            let pinned = pin!(&mut rx);
            pinned.poll_next(ctx)
        }));

        let sender = Sender::new(send, force_send);
        let receiver = Receiver::new(recv);

        Self::new(sender, receiver)
    }

    pub fn break_up(self) -> (Sender<A>, Receiver<A>) {
        (self.sender, self.receiver)
    }
}
