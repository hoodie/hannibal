use futures::{channel::oneshot, Future, FutureExt};
use std::{pin::Pin, task::Poll};

use crate::{
    actor::{Actor, Handler},
    channel::{ChanTx, ChannelWrapper},
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
    tx: ChanTx<A>,
    running: RunningFuture,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Addr {
            tx: self.tx.clone(),
            running: self.running.clone(),
        }
    }
}

impl<A: Actor> Addr<A> {
    pub fn stop(&mut self) -> Result<()> {
        self.tx.send(Payload::Stop)?;
        Ok(())
    }

    pub fn stopped(&self) -> bool {
        self.running.peek().is_some()
    }

    pub async fn call<M: Message>(&self, msg: M) -> Result<M::Result>
    where
        A: Handler<M>,
    {
        let (repsonse_tx, response) = oneshot::channel();
        self.tx.send(Payload::task(move |actor, ctx| {
            Box::pin(async move {
                let res = Handler::handle(actor, ctx, msg).await;
                let _ = repsonse_tx.send(res);
            })
        }))?;

        Ok(response.await?)
    }

    pub fn send<M: Message<Result = ()>>(&self, msg: M) -> Result<()>
    where
        A: Handler<M>,
    {
        self.tx.send(Payload::task(move |actor, ctx| {
            Box::pin(Handler::handle(actor, ctx, msg))
        }))?;
        Ok(())
    }
}

impl<A> std::future::Future for Addr<A> {
    type Output = Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        self.get_mut()
            .running
            .poll_unpin(cx)
            .map(|p| p.map_err(Into::into))
    }
}

pub fn start<A: Actor>(mut actor: A) -> (impl Future<Output = A>, Addr<A>) {
    let channel = ChannelWrapper::unbounded();
    let (mut ctx, stopped) = Context::new(&channel);
    let (tx, mut rx) = channel.break_up();

    let addr = Addr {
        tx,
        running: ctx.running.clone(),
    };

    let actor_loop = async move {
        while let Some(event) = rx.recv().await {
            match event {
                Payload::Task(f) => f(&mut actor, &mut ctx).await,
                Payload::Stop => {
                    break;
                }
            }
        }

        actor.stopped(&mut ctx).await;

        stopped.notify();
        actor
    };

    (actor_loop, addr)
}
