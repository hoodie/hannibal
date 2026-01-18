use crate::{
    Actor, Addr, Handler, Message, WeakCaller, WeakSender,
    channel::WeakTx,
    context::Core,
    error::{ActorError::ActorDropped, Result},
};

/// A weak reference to an actor.
///
/// This is the weak counterpart to [`Addr`]. It can be upgraded to a strong [`Addr`] if the Actor is still alive.
#[derive(Clone)]
pub struct WeakAddr<A: Actor> {
    core: Core,
    pub(super) weak_tx: WeakTx<A>,
}

impl<A: Actor> WeakAddr<A> {
    /// Attempts to upgrade this weak address to a strong address.
    pub fn upgrade(&self) -> Option<Addr<A>> {
        self.weak_tx.upgrade().map(|tx| Addr {
            core: self.core.clone(),
            tx,
        })
    }

    /// Returns true if the actor is still running.
    pub fn running(&self) -> bool {
        self.core.running()
    }

    /// Checks if the actor is stopped.
    pub fn stopped(&self) -> bool {
        self.core.stopped()
    }

    /// Attempts to send a stop signal to the actor.
    ///
    /// If the actor is already stopped, an error is returned.
    pub fn try_stop(&mut self) -> Result<()> {
        if let Some(mut addr) = self.upgrade() {
            addr.stop()
        } else {
            Err(ActorDropped)
        }
    }

    /// Attempts to halt the actor and awaits its termination.
    ///
    /// If the actor is already stopped, an error is returned.
    pub async fn try_halt(&mut self) -> Result<()> {
        if let Some(mut addr) = self.upgrade() {
            addr.stop()?;
            addr.await
        } else {
            Err(ActorDropped)
        }
    }

    pub(crate) const fn new(core: Core, weak_tx: WeakTx<A>) -> Self {
        WeakAddr { core, weak_tx }
    }

    /// Creates a weak sender for the actor.
    pub fn sender<M>(&self) -> WeakSender<M>
    where
        A: Actor + Handler<M>,
        M: Message<Response = ()>,
    {
        WeakSender::from_weak_tx(self.weak_tx.clone(), self.core.clone())
    }

    /// Creates a weak caller for the actor.
    pub fn caller<M>(&self) -> WeakCaller<M>
    where
        A: Actor + Handler<M>,
        M: Message,
    {
        WeakCaller::from_weak_tx(self.weak_tx.clone(), self.core.clone())
    }
}

impl<A: Actor> From<&Addr<A>> for WeakAddr<A> {
    fn from(addr: &Addr<A>) -> Self {
        let weak_tx = addr.tx.downgrade();
        let core = addr.core.clone();

        WeakAddr { core, weak_tx }
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

        let weak_addr = WeakAddr::from(&addr);
        assert_eq!(weak_addr.upgrade().unwrap().call(Add(1, 2)).await, Ok(3))
    }

    #[test_log::test(tokio::test)]
    async fn does_not_prolong_life() {
        let (event_loop, addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_addr = WeakAddr::from(&addr);
        weak_addr.upgrade().unwrap();

        drop(addr);

        actor.await.unwrap().unwrap();
        assert!(weak_addr.upgrade().is_none());
    }

    #[test_log::test(tokio::test)]
    async fn send_fails_after_drop() {
        let (event_loop, addr) = start(MyActor::default());
        let actor = tokio::spawn(event_loop);

        let weak_addr = WeakAddr::from(&addr);
        let mut addr = weak_addr.upgrade().unwrap();
        addr.stop().unwrap();

        actor.await.unwrap().unwrap();
        assert!(addr.send(Store("password")).await.is_err());
    }
}
