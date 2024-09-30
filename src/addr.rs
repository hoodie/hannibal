use futures::{channel::oneshot, FutureExt};
use std::{future::Future, pin::Pin, sync::Arc, task::Poll};

use crate::{
    actor::{Actor, Handler},
    channel::ChanTx,
    context::{Context, RunningFuture},
    error::Result,
};

type TaskFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

type TaskFn<A> =
    Box<dyn for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> TaskFuture<'a> + Send + 'static>;

pub(crate) enum Payload<A> {
    Task(TaskFn<A>),
    Stop,
}

impl<A: Actor> Payload<A> {
    pub fn task<F>(f: F) -> Self
    where
        F: for<'a> FnOnce(&'a mut A, &'a mut Context<A>) -> TaskFuture<'a> + Send + 'static,
    {
        Self::Task(Box::new(f))
    }
}

pub trait Message: 'static + Send {
    type Result: 'static + Send;
}

pub struct Addr<A> {
    pub(crate) payload_tx: ChanTx<A>,
    pub(crate) running: RunningFuture,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Addr {
            payload_tx: Arc::clone(&self.payload_tx),
            running: self.running.clone(),
        }
    }
}

impl<A: Actor> Addr<A> {
    pub fn stop(&mut self) -> Result<()> {
        self.payload_tx.send(Payload::Stop)?;
        Ok(())
    }

    pub fn stopped(&self) -> bool {
        self.running.peek().is_some()
    }

    pub async fn call<M: Message>(&self, msg: M) -> Result<M::Result>
    where
        A: Handler<M>,
    {
        let (tx_response, response) = oneshot::channel();
        self.payload_tx.send(Payload::task(move |actor, ctx| {
            Box::pin(async move {
                let res = Handler::handle(actor, ctx, msg).await;
                let _ = tx_response.send(res);
            })
        }))?;

        Ok(response.await?)
    }

    pub fn send<M: Message<Result = ()>>(&self, msg: M) -> Result<()>
    where
        A: Handler<M>,
    {
        self.payload_tx.send(Payload::task(move |actor, ctx| {
            Box::pin(Handler::handle(actor, ctx, msg))
        }))?;
        Ok(())
    }
}

impl<A> Future for Addr<A> {
    type Output = Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        self.get_mut()
            .running
            .poll_unpin(cx)
            .map(|p| p.map_err(Into::into))
    }
}
