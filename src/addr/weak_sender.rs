use dyn_clone::DynClone;

use crate::{Actor, Handler, channel, context::ContextID, error::ActorError::AlreadyStopped};

use super::{Addr, Message, Result, sender::Sender};

/// A weak reference to an actor that can receive a message `M`.
///
/// This is the weak counterpart to [`Sender`].
/// It can be upgraded if the Actor is still alive.
pub struct WeakSender<M> {
    pub(super) upgrade: Box<dyn UpgradeFn<M>>,
    pub(crate) id: ContextID,
}

impl<M: Message<Response = ()>> WeakSender<M> {
    /// Attempts to upgrade this weak sender to a strong sender.
    pub fn upgrade(&self) -> Option<Sender<M>> {
        self.upgrade.upgrade()
    }

    /// Attempts to send a message to the actor.
    ///
    /// If the actor is stopped, an error is returned.
    pub async fn try_send(&self, msg: M) -> Result<()> {
        if let Some(sender) = self.upgrade.upgrade() {
            sender.send(msg).await
        } else {
            Err(AlreadyStopped)
        }
    }

    fn new<A>(tx: channel::Tx<A>, id: ContextID) -> Self
    where
        A: Actor + Handler<M>,
        M: Message<Response = ()>,
    {
        Self::from_weak_tx(tx.downgrade(), id)
    }

    pub(crate) fn from_weak_tx<A>(weak_tx: channel::WeakTx<A>, id: ContextID) -> Self
    where
        A: Actor + Handler<M>,
        M: Message<Response = ()>,
    {
        let upgrade = Box::new(move || weak_tx.upgrade().map(|tx| Sender::new(tx, id)));

        WeakSender { upgrade, id }
    }
}

impl<M: Message<Response = ()>, A> From<Addr<A>> for WeakSender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        Self::new(addr.tx.clone(), addr.context_id)
    }
}

impl<M: Message<Response = ()>, A> From<&Addr<A>> for WeakSender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: &Addr<A>) -> Self {
        Self::new(addr.tx.clone(), addr.context_id)
    }
}

impl<M: Message<Response = ()>> Clone for WeakSender<M> {
    fn clone(&self) -> Self {
        WeakSender {
            upgrade: dyn_clone::clone_box(&*self.upgrade),
            id: self.id,
        }
    }
}

pub(super) trait UpgradeFn<M: Message<Response = ()>>:
    Send + Sync + 'static + DynClone
{
    fn upgrade(&self) -> Option<Sender<M>>;
}

impl<F, M> UpgradeFn<M> for F
where
    F: Fn() -> Option<Sender<M>>,
    F: 'static + Send + Sync + Clone,
    M: Message<Response = ()>,
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
