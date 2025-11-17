use dyn_clone::DynClone;

use crate::{
    Actor, Addr,
    context::{ContextID, RunningFuture},
    error::{ActorError::AlreadyStopped, Result},
};

/// A weak reference to an actor.
///
/// This is the weak counterpart to [`Addr`]. It can be upgraded to a strong [`Addr]` if to the Actor is still alive.
pub struct WeakAddr<A: Actor> {
    pub(crate) context_id: ContextID,
    pub(super) upgrade: Box<dyn UpgradeFn<A>>,
    pub(crate) running: RunningFuture,
}

impl<A: Actor> WeakAddr<A> {
    /// Attempts to upgrade this weak address to a strong address.
    pub fn upgrade(&self) -> Option<Addr<A>> {
        self.upgrade.upgrade()
    }

    /// Checks if the actor is stopped.
    pub fn stopped(&self) -> bool {
        self.running.peek().is_some()
    }

    /// Attempts to send a stop signal to the actor.
    ///
    /// If the actor is already stopped, an error is returned.
    pub fn try_stop(&mut self) -> Result<()> {
        if let Some(mut addr) = self.upgrade() {
            addr.stop()
        } else {
            Err(AlreadyStopped)
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
            Err(AlreadyStopped)
        }
    }
}

impl<A: Actor> From<&Addr<A>> for WeakAddr<A> {
    fn from(addr: &Addr<A>) -> Self {
        let weak_sender = addr.sender.downgrade();
        let context_id = addr.context_id;
        let running = addr.running.clone();
        let running_inner = addr.running.clone();
        let upgrade = Box::new(move || {
            let running = running_inner.clone();
            weak_sender.upgrade().map(|sender| Addr {
                context_id,
                sender,
                running,
            })
        });

        WeakAddr {
            context_id,
            upgrade,
            running,
        }
    }
}

impl<A: Actor> Clone for WeakAddr<A> {
    fn clone(&self) -> Self {
        WeakAddr {
            context_id: self.context_id,
            upgrade: dyn_clone::clone_box(&*self.upgrade),
            running: self.running.clone(),
        }
    }
}

pub(super) trait UpgradeFn<A: Actor>: Send + Sync + 'static + DynClone {
    fn upgrade(&self) -> Option<Addr<A>>;
}

impl<F, A> UpgradeFn<A> for F
where
    F: Fn() -> Option<Addr<A>>,
    F: 'static + Send + Sync + Clone,
    A: Actor,
{
    fn upgrade(&self) -> Option<Addr<A>> {
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
