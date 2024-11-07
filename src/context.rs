use std::{future::Future, time::Duration};

use futures::{channel::oneshot, FutureExt};
use slab::Slab;

use crate::{
    actor::{spawn_strategy::Spawner, Actor},
    channel::WeakChanTx,
    environment::Payload,
    error::{ActorError::AlreadyStopped, Result},
    prelude::Spawnable,
    Handler, RestartableActor, Sender, WeakSender,
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
    pub(crate) tasks: Slab<futures::future::AbortHandle>,
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

    pub fn create_child<F, C, S>(&mut self, create_child: F)
    where
        F: FnOnce() -> C,
        C: Actor + Spawnable<S>,
        C: Handler<()>,
        S: Spawner<C>,
    {
        let child_addr = create_child().spawn();
        self.add_child(child_addr);
    }
}

use crate::actor::spawn_strategy::SpawnableHack as _;

impl<A> Context<A>
where
    A: Actor,
{
    pub fn spawn_task(&mut self, task: impl Future<Output = ()> + Send + 'static) {
        let (handle, registration) = futures::future::AbortHandle::new_pair();
        self.tasks.insert(handle);

        A::spawn_future(futures::stream::Abortable::new(task, registration).map(|_| ()))
    }

    pub fn start_interval_of<M: crate::Message<Result = ()>>(
        &mut self,
        create_message: impl Fn() -> M + Send + Sync + 'static,
        duration: Duration,
    ) where
        A: Handler<M>,
    {
        let sender = WeakSender::from_weak_tx(std::sync::Weak::clone(&self.weak_tx));

        self.spawn_task(async move {
            loop {
                A::sleep(duration).await;
                if sender.try_send(create_message()).is_err() {
                    break;
                }
            }
        })
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
