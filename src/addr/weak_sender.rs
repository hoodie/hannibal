use dyn_clone::DynClone;

use std::sync::Arc;

use crate::{
    channel::{ChanTx, ForceChanTx, WeakChanTx, WeakForceChanTx},
    context::ContextID,
    error::ActorError::AlreadyStopped,
    Actor, Handler,
};

use super::{sender::Sender, Addr, Message, Result};

pub struct WeakSender<M> {
    pub(super) upgrade: Box<dyn UpgradeFn<M>>,
    pub(crate) id: ContextID,
}

impl<M: Message<Result = ()>> WeakSender<M> {
    pub fn upgrade(&self) -> Option<Sender<M>> {
        self.upgrade.upgrade()
    }

    pub fn try_force_send(&self, msg: M) -> Result<()> {
        if let Some(sender) = self.upgrade.upgrade() {
            #[allow(deprecated)] // just as deprecated
            sender.force_send(msg)
        } else {
            Err(AlreadyStopped)
        }
    }

    pub async fn try_send(&self, msg: M) -> Result<()> {
        if let Some(sender) = self.upgrade.upgrade() {
            sender.send(msg).await
        } else {
            Err(AlreadyStopped)
        }
    }

    fn new<A>(tx: ChanTx<A>, force_tx: ForceChanTx<A>, id: ContextID) -> Self
    where
        A: Actor + Handler<M>,
        M: Message<Result = ()>,
    {
        Self::from_weak_tx(Arc::downgrade(&tx), Arc::downgrade(&force_tx), id)
    }

    pub(crate) fn from_weak_tx<A>(
        weak_tx: WeakChanTx<A>,
        weak_force_tx: WeakForceChanTx<A>,
        id: ContextID,
    ) -> Self
    where
        A: Actor + Handler<M>,
        M: Message<Result = ()>,
    {
        let upgrade = Box::new(move || {
            weak_tx
                .upgrade()
                .zip(weak_force_tx.upgrade())
                .map(|(tx, force_tx)| Sender::new(tx, force_tx, id))
        });

        WeakSender { upgrade, id }
    }
}

impl<M: Message<Result = ()>, A> From<Addr<A>> for WeakSender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        Self::new(
            addr.payload_tx.to_owned(),
            addr.payload_force_tx.to_owned(),
            addr.context_id,
        )
    }
}

impl<M: Message<Result = ()>, A> From<&Addr<A>> for WeakSender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: &Addr<A>) -> Self {
        Self::new(
            addr.payload_tx.to_owned(),
            addr.payload_force_tx.to_owned(),
            addr.context_id,
        )
    }
}

impl<M: Message<Result = ()>> Clone for WeakSender<M> {
    fn clone(&self) -> Self {
        WeakSender {
            upgrade: dyn_clone::clone_box(&*self.upgrade),
            id: self.id,
        }
    }
}

pub(super) trait UpgradeFn<M: Message<Result = ()>>:
    Send + Sync + 'static + DynClone
{
    fn upgrade(&self) -> Option<Sender<M>>;
}

impl<F, M> UpgradeFn<M> for F
where
    F: Fn() -> Option<Sender<M>>,
    F: 'static + Send + Sync + Clone,
    M: Message<Result = ()>,
{
    fn upgrade(&self) -> Option<Sender<M>> {
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
        let (event_loop, mut addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_sender = WeakSender::from(&addr);
        weak_sender
            .upgrade()
            .unwrap()
            .send(Store("password"))
            .await
            .unwrap();
        addr.stop().unwrap();
        let actor = actor.await.unwrap().unwrap();
        assert_eq!(actor.0, Some("password"));
    }

    #[test_log::test(tokio::test)]
    async fn does_not_prolong_life() {
        let (event_loop, addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_sender: WeakSender<Store> = WeakSender::from(&addr);
        weak_sender.upgrade().unwrap();

        drop(addr);

        actor.await.unwrap().unwrap();
        assert!(weak_sender.upgrade().is_none());
    }

    #[test_log::test(tokio::test)]
    async fn try_call_fails() {
        let (event_loop, mut addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_sender: WeakSender<Store> = WeakSender::from(&addr);
        addr.stop().unwrap();

        actor.await.unwrap().unwrap();
        assert!(weak_sender.try_send(Store("to /dev/null")).await.is_err());
    }
}
