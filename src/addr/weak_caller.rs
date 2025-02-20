use dyn_clone::DynClone;

use std::sync::Arc;

use crate::{
    Actor, Handler,
    channel::{ChanTx, WeakChanTx},
    context::ContextID,
    error::ActorError::AlreadyStopped,
};

use super::{Addr, Message, Result, caller::Caller};

/// A weak reference to an actor that can receive `M`.
///
/// This is the weak counterpart to [`Caller`].
/// It can be upgraded if the Actor is still alive.
pub struct WeakCaller<M: Message> {
    pub(super) upgrade: Box<dyn UpgradeFn<M>>,
    pub(crate) id: ContextID,
}

impl<M: Message> WeakCaller<M> {
    pub fn upgrade(&self) -> Option<Caller<M>> {
        self.upgrade.upgrade()
    }

    pub async fn try_call(&self, msg: M) -> Result<M::Response> {
        if let Some(caller) = self.upgrade.upgrade() {
            caller.call(msg).await
        } else {
            Err(AlreadyStopped)
        }
    }

    fn new<A>(tx: ChanTx<A>, id: ContextID) -> Self
    where
        A: Actor + Handler<M>,
        M: Message,
    {
        Self::from_weak_tx(Arc::downgrade(&tx), id)
    }

    pub(crate) fn from_weak_tx<A>(weak_tx: WeakChanTx<A>, id: ContextID) -> Self
    where
        A: Actor + Handler<M>,
        M: Message,
    {
        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Caller::new(tx, id)));

        WeakCaller { upgrade, id }
    }
}

impl<M: Message, A> From<Addr<A>> for WeakCaller<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        Self::new(addr.payload_tx.to_owned(), addr.context_id)
    }
}

impl<M: Message, A> From<&Addr<A>> for WeakCaller<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: &Addr<A>) -> Self {
        Self::new(addr.payload_tx.to_owned(), addr.context_id)
    }
}

impl<M: Message> Clone for WeakCaller<M> {
    fn clone(&self) -> Self {
        WeakCaller {
            upgrade: dyn_clone::clone_box(&*self.upgrade),
            id: self.id,
        }
    }
}

pub(super) trait UpgradeFn<M: Message>: Send + Sync + 'static + DynClone {
    fn upgrade(&self) -> Option<Caller<M>>;
}

impl<F, M> UpgradeFn<M> for F
where
    F: Fn() -> Option<Caller<M>>,
    F: 'static + Send + Sync + Clone,
    M: Message,
{
    fn upgrade(&self) -> Option<Caller<M>> {
        self()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::addr::tests::*;

    #[test_log::test(tokio::test)]
    async fn upgrade() {
        let (event_loop, addr) = start(MyActor::default());
        tokio::spawn(event_loop);

        let weak_caller = WeakCaller::from(&addr);
        assert_eq!(weak_caller.upgrade().unwrap().call(Add(1, 2)).await, Ok(3))
    }

    #[test_log::test(tokio::test)]
    async fn does_not_prolong_life() {
        let (event_loop, addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_caller: WeakCaller<Add> = WeakCaller::from(&addr);
        weak_caller.upgrade().unwrap();

        drop(addr);

        actor.await.unwrap().unwrap();
        assert!(weak_caller.upgrade().is_none());
    }

    #[test_log::test(tokio::test)]
    async fn try_call_fails() {
        let (event_loop, mut addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_caller: WeakCaller<Add> = WeakCaller::from(&addr);
        addr.stop().unwrap();

        actor.await.unwrap().unwrap();
        assert!(weak_caller.try_call(Add(1, 2)).await.is_err());
    }
}
