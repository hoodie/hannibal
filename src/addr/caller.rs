use futures::channel::oneshot;

use dyn_clone::DynClone;
use std::{future::Future, pin::Pin, sync::Arc};

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
    upgrade: Box<dyn UpgradeFn<M>>,
}

impl<M: Message> WeakCaller<M> {
    pub fn upgrade(&self) -> Option<Caller<M>> {
        self.upgrade.upgrade()
    }

    pub async fn try_call(&self, msg: M) -> Option<Result<M::Result>> {
        if let Some(caller) = self.upgrade.upgrade() {
            Some(caller.call(msg).await)
        } else {
            None
        }
    }
}

impl<M, A> From<ChanTx<A>> for WeakCaller<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn from(tx: ChanTx<A>) -> Self {
        let weak_tx = Arc::downgrade(&tx);

        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Caller::from(tx)));

        WeakCaller { upgrade }
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
            upgrade: dyn_clone::clone_box(&*self.upgrade),
        }
    }
}

trait UpgradeFn<M: Message>: Send + Sync + 'static + DynClone {
    fn upgrade(&self) -> Option<Caller<M>>;
}

impl<F, M> UpgradeFn<M> for F
where
    F: Fn() -> Option<Caller<M>>,
    F: 'static + Send + Sync + DynClone,
    M: Message,
{
    fn upgrade(&self) -> Option<Caller<M>> {
        self()
    }
}
