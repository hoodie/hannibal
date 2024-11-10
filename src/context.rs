use futures::channel::oneshot;

use crate::{
    actor::{spawn_strategy::Spawner, Actor},
    channel::WeakChanTx,
    environment::Payload,
    error::{ActorError::AlreadyStopped, Result},
    prelude::Spawnable,
    Handler, Message, RestartableActor, Sender,
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
    pub(crate) children: Vec<Sender<()>>,
    pub(crate) tasks: Vec<futures::future::AbortHandle>,
}

impl<A> Drop for Context<A> {
    fn drop(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
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
        self.children.push(child.into());
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

    pub(crate) fn weak_sender<M: Message<Result = ()>>(&self) -> crate::WeakSender<M>
    where
        A: Handler<M>,
    {
        crate::WeakSender::from_weak_tx(std::sync::Weak::clone(&self.weak_tx))
    }
}

#[cfg(any(feature = "tokio", feature = "async-std", feature = "custom_runtime"))]
mod task_handling {
    use futures::FutureExt;
    use std::{future::Future, time::Duration};

    use crate::{actor::Actor, spawn_strategy::SpawnableHack, Context, Handler, Message};

    /// Task Handling
    impl<A: Actor> Context<A> {
        fn spawn_task(&mut self, task: impl Future<Output = ()> + Send + 'static) {
            let (task, handle) = futures::future::abortable(task);

            self.tasks.push(handle);
            A::spawn_future(task.map(|_| ()))
        }

        pub fn interval<M: Message<Result = ()> + Clone>(&mut self, message: M, duration: Duration)
        where
            A: Handler<M>,
        {
            let myself = self.weak_sender();
            self.spawn_task(async move {
                loop {
                    A::sleep(duration).await;
                    if myself.try_send(message.clone()).is_err() {
                        break;
                    }
                }
            })
        }

        pub fn interval_with<M: Message<Result = ()>>(
            &mut self,
            message_fn: impl Fn() -> M + Send + Sync + 'static,
            duration: Duration,
        ) where
            A: Handler<M>,
        {
            let myself = self.weak_sender();
            self.spawn_task(async move {
                loop {
                    A::sleep(duration).await;
                    if myself.try_send(message_fn()).is_err() {
                        break;
                    }
                }
            })
        }

        pub fn delayed<M: Message<Result = ()>>(
            &mut self,
            message_fn: impl Fn() -> M + Send + Sync + 'static,
            duration: Duration,
        ) where
            A: Handler<M>,
        {
            let myself = self.weak_sender();
            self.spawn_task(async move {
                A::sleep(duration).await;

                if myself.try_send(message_fn()).is_err() {
                    // log::warn!("Failed to send message");
                }
            })
        }
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
