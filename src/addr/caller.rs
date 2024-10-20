use dyn_clone::DynClone;
use futures::channel::oneshot;

use std::sync::{Arc, Weak};
use std::{future::Future, pin::Pin};

use crate::{channel::ChanTx, Actor, Handler};

use super::{weak_caller::WeakCaller, Addr, Message, Payload, Result};

pub struct Caller<M: Message> {
    call_fn: Box<dyn CallerFn<M>>,
    downgrade_fn: Box<dyn DowngradeFn<M>>,
}

impl<M: Message> Caller<M> {
    pub async fn call(&self, msg: M) -> Result<M::Result> {
        self.call_fn.call(msg).await
    }

    pub fn downgrade(&self) -> WeakCaller<M> {
        self.downgrade_fn.downgrade()
    }

    pub(crate) fn from_tx<A>(tx: ChanTx<A>) -> Self
    where
        A: Actor + Handler<M>,
    {
        let weak_tx: Weak<_> = Arc::downgrade(&tx);

        let call_fn = Box::new(
            move |msg| -> Pin<Box<dyn Future<Output = Result<M::Result>>>> {
                let tx = Arc::clone(&tx);
                Box::pin(async move {
                    let (response_tx, response) = oneshot::channel();

                    tx.send(Payload::task(move |actor, ctx| {
                        Box::pin(async move {
                            let res = Handler::handle(&mut *actor, ctx, msg).await;
                            let _ = response_tx.send(res);
                        })
                    }))?;

                    Ok(response.await?)
                })
            },
        );

        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Caller::from_tx(tx)));

        let downgrade_fn = Box::new(move || WeakCaller {
            upgrade: upgrade.clone(),
        });

        Caller {
            call_fn,
            downgrade_fn,
        }
    }
}

trait CallerFn<M: Message>: Send + Sync + 'static + DynClone {
    fn call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>>;
}

impl<F, M> CallerFn<M> for F
where
    F: Fn(M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>>,
    F: 'static + Send + Sync + Clone,
    M: Message,
{
    fn call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>> {
        self(msg)
    }
}

impl<M: Message, A> From<Addr<A>> for Caller<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        Caller::from_tx(addr.payload_tx.to_owned())
    }
}

impl<M: Message> Clone for Caller<M> {
    fn clone(&self) -> Self {
        Caller {
            call_fn: dyn_clone::clone_box(&*self.call_fn),
            downgrade_fn: dyn_clone::clone_box(&*self.downgrade_fn),
        }
    }
}

trait DowngradeFn<M: Message>: Send + Sync + 'static + DynClone {
    fn downgrade(&self) -> WeakCaller<M>;
}

impl<F, M> DowngradeFn<M> for F
where
    F: Fn() -> WeakCaller<M>,
    F: 'static + Send + Sync + DynClone,
    M: Message,
{
    fn downgrade(&self) -> WeakCaller<M> {
        self()
    }
}
