use futures::channel::oneshot;

use dyn_clone::DynClone;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
};

use crate::{channel::ChanTx, Actor, Handler};

use super::{ActorEvent, Message, Result};

trait CallerFn<M: Message>: Send + Sync + 'static + DynClone {
    fn call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>>;
}

impl<F, M> CallerFn<M> for F
where
    F: Fn(M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>>,
    F: 'static + Send + Sync + DynClone,
    M: Message,
{
    fn call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>> {
        self(msg)
    }
}

/// Caller of a specific message type.
pub struct Caller<M: Message> {
    call_fn: Box<dyn CallerFn<M>>,
}

impl<M: Message> Caller<M> {
    pub async fn call(&self, msg: M) -> Result<M::Result> {
        self.call_fn.call(msg).await
    }
}

impl<M, A> From<ChanTx<A>> for Caller<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn from(tx: ChanTx<A>) -> Self {
        let call_fn = Box::new(move |msg| {
            let tx: Arc<_> = tx.clone();
            Box::pin(async move {
                let (response_tx, response) = oneshot::channel();

                tx.send(ActorEvent::exec(move |actor, ctx| {
                    Box::pin(async move {
                        let res = Handler::handle(&mut *actor, ctx, msg).await;
                        let _ = response_tx.send(res);
                    })
                }))?;

                Ok(response.await?)
            }) as Pin<Box<dyn Future<Output = Result<M::Result>>>>
        });

        Caller { call_fn }
    }
}

/// Caller of a specific message type.
pub struct WeakCaller<M: Message> {
    call_fn: Box<dyn CallerFn<M>>,
}

impl<M> WeakCaller<M>
where
    M: Message,
{
    pub fn try_call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>> {
        self.call_fn.call(msg)
    }
}

impl<M, A> From<ChanTx<A>> for WeakCaller<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn from(tx: ChanTx<A>) -> Self {
        let call_fn = Box::new(move |msg| {
            let wtx: Weak<_> = Arc::downgrade(&tx);
            Box::pin(async move {
                let Some(tx) = wtx.upgrade() else {
                    return Err(anyhow::anyhow!("failed to upgrade"));
                };
                let (response_tx, response) = oneshot::channel();

                tx.send(ActorEvent::exec(move |actor, ctx| {
                    Box::pin(async move {
                        let res = Handler::handle(&mut *actor, ctx, msg).await;
                        let _ = response_tx.send(res);
                    })
                }))?;

                Ok(response.await?)
            }) as Pin<Box<dyn Future<Output = Result<M::Result>>>>
        });

        WeakCaller { call_fn }
    }
}

impl<M: Message> Clone for Caller<M> {
    fn clone(&self) -> Self {
        Caller {
            call_fn: dyn_clone::clone_box(&*self.call_fn),
        }
    }
}

impl<M: Message> Clone for WeakCaller<M> {
    fn clone(&self) -> Self {
        WeakCaller {
            call_fn: dyn_clone::clone_box(&*self.call_fn),
        }
    }
}
