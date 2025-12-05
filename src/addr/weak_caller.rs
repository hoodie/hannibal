use dyn_clone::DynClone;

use crate::{Actor, Handler, channel, context::Core, error::ActorError::AlreadyStopped};

use super::{Addr, Message, Result, caller::Caller};

/// A weak reference to an actor that can receive a message `M` and respond.
///
/// This is the weak counterpart to [`Caller`].
/// It can be upgraded if the Actor is still alive.
pub struct WeakCaller<M: Message> {
    pub(crate) core: Core,
    pub(super) upgrade: Box<dyn UpgradeFn<M>>,
}

impl<M: Message> WeakCaller<M> {
    /// Attempts to upgrade this weak caller to a strong caller.
    pub fn upgrade(&self) -> Option<Caller<M>> {
        self.upgrade.upgrade()
    }

    /// Attempts to call the actor and receive a response.
    ///
    /// If the actor is stopped, an error is returned.
    pub async fn upgrade_and_call(&self, msg: M) -> Result<M::Response> {
        if let Some(caller) = self.upgrade.upgrade() {
            caller.call(msg).await
        } else {
            Err(AlreadyStopped)
        }
    }

    fn new<A>(tx: channel::Tx<A>, core: Core) -> Self
    where
        A: Actor + Handler<M>,
        M: Message,
    {
        Self::from_weak_tx(tx.downgrade(), core)
    }

    pub(crate) fn from_weak_tx<A>(weak_tx: channel::WeakTx<A>, core: Core) -> Self
    where
        A: Actor + Handler<M>,
        M: Message,
    {
        let upgrade = {
            let core = core.clone();
            Box::new(move || weak_tx.upgrade().map(|tx| Caller::new(tx, core.clone())))
        };

        WeakCaller { core, upgrade }
    }
}

impl<M: Message> WeakCaller<M> {
    /// Returns true if the actor is still running.
    pub fn running(&self) -> bool {
        self.core.running()
    }

    /// Returns true if the actor has stopped.
    pub fn stopped(&self) -> bool {
        self.core.stopped()
    }
}
impl<M: Message, A> From<Addr<A>> for WeakCaller<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: Addr<A>) -> Self {
        Self::new(addr.tx.clone(), addr.core)
    }
}

impl<M: Message, A> From<&Addr<A>> for WeakCaller<M>
where
    A: Actor + Handler<M>,
{
    fn from(addr: &Addr<A>) -> Self {
        Self::new(addr.tx.clone(), addr.core.clone())
    }
}

impl<M: Message> Clone for WeakCaller<M> {
    fn clone(&self) -> Self {
        WeakCaller {
            core: self.core.clone(),
            upgrade: dyn_clone::clone_box(&*self.upgrade),
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
        assert!(weak_caller.upgrade_and_call(Add(1, 2)).await.is_err());
    }
}
