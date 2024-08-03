#![allow(unused_imports)]
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
};

use futures::{stream::FusedStream, StreamExt};

use crate::addr::ActorEvent;

type EmptyResult = anyhow::Result<()>;
type Result<T> = anyhow::Result<T>;

mod fns {
    use std::pin::Pin;

    use dyn_clone::DynClone;

    use super::*;
    pub(crate) trait SendFn<A>: Send + Sync {
        fn call(&self, msg: ActorEvent<A>) -> EmptyResult;
    }

    impl<F, A> SendFn<A> for F
    where
        F: Fn(ActorEvent<A>) -> EmptyResult,
        F: Send + Sync,
    {
        fn call(&self, msg: ActorEvent<A>) -> EmptyResult {
            self(msg)
        }
    }

    pub(crate) trait RecvFn<A>: Send + Sync + DynClone {
        fn call(
            &mut self,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<ActorEvent<A>>> + Send>>;
    }

    impl<F, A> RecvFn<A> for F
    where
        F: FnMut() -> Pin<Box<dyn std::future::Future<Output = Result<ActorEvent<A>>> + Send>>,
        F: Send + Sync,
        F: DynClone,
    {
        fn call(
            &mut self,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<ActorEvent<A>>> + Send>> {
            self()
        }
    }
}

pub(crate) use fns::{RecvFn, SendFn};

pub struct ChannelWrapper<A> {
    // TODO: should we hold ARCs to tx and rx here to ensure lifeness of the channel?
    // in a normal channel the TX is strong and the RX is weak, this applies here as
    // well since we want the actor to shut down once all addresses (senders) have gone away
    send_fn: Arc<dyn SendFn<A>>,
    recv_fn: Option<Box<dyn RecvFn<A>>>,
}

impl<A> ChannelWrapper<A> {
    fn wrap(send_fn: Arc<dyn SendFn<A>>, recv_fn: Box<dyn RecvFn<A>>) -> Self {
        ChannelWrapper {
            send_fn,
            recv_fn: recv_fn.into(),
        }
    }
    pub fn send(&self, msg: ActorEvent<A>) -> EmptyResult {
        self.send_fn.call(msg)
    }

    // pub async fn receive(&mut self) -> Result<ActorEvent<A>> {
    //     Ok(self.recv_fn.call().await?)
    // }
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
        let send = Arc::new(move |event: ActorEvent<A>| -> EmptyResult {
            let mut tx = tx.clone();
            tx.start_send(event)?;
            Ok(())
        });

        let rx = Arc::new(async_lock::Mutex::new(rx));
        let recv = Box::new(move || {
            let rx = rx.clone();
            Box::pin(async move {
                dbg!(Arc::strong_count(&rx));
                let mut rx = rx.lock().await;
                let next = rx.next();
                next.await.ok_or(anyhow::anyhow!("stream ran dry"))
            }) as Pin<Box<dyn Future<Output = Result<ActorEvent<A>>> + Send>>
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
        let send = Arc::new(move |event: ActorEvent<A>| -> anyhow::Result<()> {
            let mut wtx = tx.clone();
            wtx.start_send(event)?;
            Ok(())
        });

        let rx = Arc::new(async_lock::Mutex::new(rx));
        let recv = Box::new(move || {
            let rx = rx.clone();
            Box::pin(async move {
                dbg!(Arc::strong_count(&rx));
                let mut rx = rx.lock().await;
                let next = rx.next();
                next.await.ok_or(anyhow::anyhow!("stream ran dry"))
            }) as Pin<Box<dyn Future<Output = Result<ActorEvent<A>>> + Send>>
        });
        Self::wrap(send, recv)
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

    /// Returns a strong reference to the send function
    /// This is for the [`Addr`] (and for the [`LifeCycle`] to create new [`Addr`]esses)
    pub fn tx(&self) -> Arc<dyn SendFn<A>> {
        self.send_fn.clone()
    }

    /// Returns a weak reference to the send function
    /// This is for the [`Context`]
    pub fn weak_tx(&self) -> Weak<dyn SendFn<A>> {
        Arc::downgrade(&self.send_fn)
    }

    /// This should be held exclusively by the [`LifeCycle`]
    pub fn rx(&mut self) -> Option<Box<dyn RecvFn<A>>> {
        // dyn_clone::clone_box(&*self.recv_fn)
        self.recv_fn.take()
    }

    // #[deprecated(note = "nobody needs this")]
    // pub fn weak_rx(&self) -> Weak<dyn SendFn<A>> {
    //     Arc::downgrade(&self.send_fn)
    // }
}
