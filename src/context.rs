use futures::channel::oneshot;
use slab::Slab;

use crate::{
    actor::{spawn_strategy::Spawner, Actor},
    channel::WeakChanTx,
    environment::Payload,
    error::{ActorError::AlreadyStopped, Result},
    prelude::Spawnable,
    Handler, RestartableActor, Sender,
};

pub type RunningFuture = futures::future::Shared<oneshot::Receiver<()>>;
pub struct StopNotifier(pub(crate) oneshot::Sender<()>);
impl StopNotifier {
    pub fn notify(self) {
        self.0.send(()).ok();
    }
}

pub struct Context<A> {
    pub(crate) weak_tx: WeakChanTx<A>,
    pub(crate) running: RunningFuture,
    pub(crate) children: Slab<Sender<()>>,
}

impl<A: Actor> Context<A> {
    pub fn stop(&self) -> Result<()> {
        if let Some(tx) = self.weak_tx.upgrade() {
            Ok(tx.send(Payload::Stop)?)
        } else {
            Err(AlreadyStopped)
        }
    }

    pub fn add_child(&mut self, child: impl Into<Sender<()>>) {
        self.children.insert(child.into());
    }

    pub fn create_child<F, C, S>(&mut self, create_child: F) -> crate::error::Result<()>
    where
        F: FnOnce() -> C,
        C: Actor + Spawnable<S>,
        C: Handler<()>,
        S: Spawner<C>,
    {
        let child_addr = create_child().spawn()?;
        self.add_child(child_addr);
        Ok(())
    }
}

impl<A: RestartableActor> Context<A> {
    pub fn restart(&self) -> Result<()> {
        if let Some(tx) = self.weak_tx.upgrade() {
            Ok(tx.send(Payload::Restart)?)
        } else {
            Err(AlreadyStopped)
        }
    }
}
