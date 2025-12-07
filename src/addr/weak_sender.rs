use dyn_clone::DynClone;

use crate::{Actor, Handler, channel, context::Core, error::ActorError::AlreadyStopped};

use super::{Addr, Message, Result, sender::Sender};

/// A weak reference to an actor that can receive a message `M`.
///
/// This is the weak counterpart to [`Sender`].
/// It can be upgraded if the Actor is still alive.
pub struct WeakSender<M> {
    pub(crate) core: Core,
    pub(super) upgrade: Box<dyn UpgradeFn<M>>,
}

impl<M: Message<Response = ()>> WeakSender<M> {
    /// Attempts to upgrade this weak sender to a strong sender.
    pub fn upgrade(&self) -> Option<Sender<M>> {
        self.upgrade.upgrade()
    }

    /// Attempts to send a message to the actor.
    ///
    /// If the actor is stopped, an error is returned.
    pub async fn upgrade_and_send(&self, msg: M) -> Result<()> {
        if let Some(sender) = self.upgrade.upgrade() {
            sender.send(msg).await
        } else {
            Err(AlreadyStopped)
        }
    }

    fn new<A>(tx: channel::Tx<A>, core: Core) -> Self
    where
        A: Actor + Handler<M>,
        M: Message<Response = ()>,
    {
        Self::from_weak_tx(tx.downgrade(), core)
    }

    pub(crate) fn from_weak_tx<A>(weak_tx: channel::WeakTx<A>, core: Core) -> Self
    where
        A: Actor + Handler<M>,
        M: Message<Response = ()>,
    {
        let upgrade = {
            let core = core.clone();
            Box::new(move || weak_tx.upgrade().map(|tx| Sender::new(tx, core.clone())))
        };

        WeakSender { core, upgrade }
    }
}

impl<M: Message<Response = ()>> WeakSender<M> {
    /// Returns true if the actor is still running.
    pub fn running(&self) -> bool {
        self.core.running()
    }

    /// Returns true if the actor has stopped.
    pub fn stopped(&self) -> bool {
        self.core.stopped()
    }
}
impl<M: Message<Response = ()>, A> From<Addr<A>> for WeakSender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        Self::new(addr.tx.clone(), addr.core)
    }
}

impl<M: Message<Response = ()>, A> From<&Addr<A>> for WeakSender<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: &Addr<A>) -> Self {
        Self::new(addr.tx.clone(), addr.core.clone())
    }
}

impl<M: Message<Response = ()>> Clone for WeakSender<M> {
    fn clone(&self) -> Self {
        WeakSender {
            core: self.core.clone(),
            upgrade: dyn_clone::clone_box(&*self.upgrade),
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
    async fn upgrade_and_send_fails() {
        let (event_loop, mut addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_sender: WeakSender<Store> = WeakSender::from(&addr);
        addr.stop().unwrap();

        actor.await.unwrap().unwrap();
        assert!(
            weak_sender
                .upgrade_and_send(Store("to /dev/null"))
                .await
                .is_err()
        );
    }
}
