use futures::channel::oneshot;

use crate::{
    Addr, Handler, RestartableActor, Sender, WeakAddr,
    actor::{Actor, spawner::Spawner},
    channel::{WeakChanTx, WeakForceChanTx},
    environment::Payload,
    error::{ActorError::AlreadyStopped, Result},
    prelude::Spawnable,
};
pub use id::ContextID;

pub type RunningFuture = futures::future::Shared<oneshot::Receiver<()>>;
pub struct StopNotifier(pub(crate) oneshot::Sender<()>);
impl StopNotifier {
    pub fn notify(self) {
        self.0.send(()).ok();
    }
}

mod id {
    use std::sync::{LazyLock, atomic::AtomicU64};
    static CONTEXT_ID: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ContextID(u64);

    impl Default for ContextID {
        fn default() -> Self {
            Self(CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        }
    }

    impl std::fmt::Display for ContextID {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    #[test]
    fn ids_go_up() {
        let id1 = ContextID::default();
        let id2 = ContextID::default();
        let id3 = ContextID::default();
        assert!(id1 < id2);
        assert!(id2 < id3);
    }
}

/// Available to the actor in every execution call.
///
/// The context is used to interact with the actor system.
/// You can start intervals, send messages to yourself, and stop the actor.
///
pub struct Context<A> {
    pub(crate) id: ContextID,
    #[allow(dead_code)]
    pub(crate) weak_tx: WeakChanTx<A>,
    pub(crate) weak_force_tx: WeakForceChanTx<A>,
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

/// Life-cycle
impl<A: Actor> Context<A> {
    /// Stop the actor.
    pub fn stop(&self) -> Result<()> {
        if let Some(tx) = self.weak_force_tx.upgrade() {
            Ok(tx.send(Payload::Stop)?)
        } else {
            Err(AlreadyStopped)
        }
    }
}

/// Child Actors
impl<A: Actor> Context<A> {
    /// Add a child actor to the context.
    ///
    /// The child actor will be stopped when the parent actor is stopped.
    pub fn add_child(&mut self, child: impl Into<Sender<()>>) {
        self.children.push(child.into());
    }

    /// Create a child actor and add it to the context.
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

impl<A: Actor> Context<A> {
    /// Create an strong address to the actor.
    ///
    /// This only works if the actor is still running, otherwise you'll get `None`.
    ///
    /// <div class="warning">Leak Potential!</div>
    ///
    /// If you store an `Addr` to the actor within itself it will no longer self terminate.
    /// This is not public since it offers no more convenience than [`Context::weak_address`] (you need to upgrade either way).
    fn address(&self) -> Option<Addr<A>> {
        let payload_tx = self.weak_tx.upgrade()?;
        let payload_force_tx = self.weak_force_tx.upgrade()?;

        Some(Addr {
            context_id: self.id,
            payload_tx,
            payload_force_tx,
            running: self.running.clone(),
        })
    }

    /// Create an week address to the actor.
    pub fn weak_address(&self) -> Option<WeakAddr<A>> {
        self.address().as_ref().map(Addr::downgrade)
    }

    /// Create a weak sender to the actor.
    #[cfg(feature = "runtime")]
    pub fn weak_sender<M: crate::Message<Response = ()>>(&self) -> crate::WeakSender<M>
    where
        A: Handler<M>,
    {
        crate::WeakSender::from_weak_tx(
            std::sync::Weak::clone(&self.weak_tx),
            std::sync::Weak::clone(&self.weak_force_tx),
            self.id,
        )
    }

    #[cfg(feature = "runtime")]
    pub fn weak_caller<M: crate::Message<Response = R>, R>(&self) -> crate::WeakCaller<M>
    where
        A: Handler<M>,
    {
        crate::WeakCaller::from_weak_tx(std::sync::Weak::clone(&self.weak_tx), self.id)
    }
}

/// Broker Interaction
impl<A: Actor> Context<A> {
    /// Publish to the broker.
    ///
    /// Every actor can publish messages to the broker
    /// which will be delivered to all actors that subscribe to the message.
    #[cfg(any(feature = "tokio_runtime", feature = "async_runtime"))]
    pub async fn publish<M: crate::Message<Response = ()> + Clone>(&self, message: M) -> Result<()>
    where
        A: Handler<M>,
    {
        crate::Broker::publish(message).await
    }

    /// Subscribe to a message.
    ///
    /// The actor will receive all messages of this type.
    #[cfg(feature = "runtime")]
    pub async fn subscribe<M: crate::Message<Response = ()> + Clone>(&mut self) -> Result<()>
    where
        A: Handler<M>,
    {
        crate::Broker::subscribe(self.weak_sender()).await
    }
}

#[cfg(feature = "runtime")]
mod task_handling {
    use futures::FutureExt;
    use std::{future::Future, time::Duration};

    use crate::{Context, Handler, Message, actor::Actor, spawner::SpawnSelf};

    /// Task Handling
    impl<A: Actor> Context<A> {
        fn spawn_task(&mut self, task: impl Future<Output = ()> + Send + 'static) {
            let (task, handle) = futures::future::abortable(task);

            self.tasks.push(handle);
            A::spawn_future(task.map(|_| ()))
        }

        #[cfg(test)]
        pub(crate) fn stop_tasks(&mut self) {
            for handle in self.tasks.drain(..) {
                handle.abort();
            }
        }

        /// Send yourself a message at a regular interval.
        pub fn interval<M: Message<Response = ()> + Clone + Send + 'static>(
            &mut self,
            message: M,
            duration: Duration,
        ) where
            A: Handler<M> + Send + 'static,
        {
            let myself = self.weak_sender();
            self.spawn_task(async move {
                loop {
                    A::sleep(duration).await;
                    if myself.try_force_send(message.clone()).is_err() {
                        break;
                    }
                }
            })
        }

        /// Send yourself a message at a regular interval.
        pub fn interval_with<M: Message<Response = ()>>(
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
                    if myself.try_send(message_fn()).await.is_err() {
                        break;
                    }
                }
            })
        }

        /// Send yourself a message after a delay.
        pub fn delayed_send<M: Message<Response = ()>>(
            &mut self,
            message_fn: impl Fn() -> M + Send + Sync + 'static,
            duration: Duration,
        ) where
            A: Handler<M>,
        {
            let myself = self.weak_sender();
            self.spawn_task(async move {
                A::sleep(duration).await;

                if myself.try_send(message_fn()).await.is_err() {
                    log::warn!("Failed to send message");
                }
            })
        }

        pub fn delayed_exec<F: Future<Output = ()> + Send + 'static>(
            &mut self,
            task: F,
            duration: Duration,
        ) {
            self.spawn_task(async move {
                A::sleep(duration).await;
                task.await;
            })
        }
    }
}

/// Life-cycle
impl<A: RestartableActor> Context<A> {
    /// Restart the actor.
    pub fn restart(&self) -> Result<()> {
        if let Some(tx) = self.weak_force_tx.upgrade() {
            Ok(tx.send(Payload::Restart)?)
        } else {
            Err(AlreadyStopped)
        }
    }
}

#[cfg(test)]
#[cfg(feature = "runtime")]
mod interval_cleanup {
    #![allow(clippy::unwrap_used)]
    #[cfg(feature = "async_runtime")]
    use async_std::task::sleep;

    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };
    #[cfg(feature = "tokio_runtime")]
    use tokio::time::sleep;

    mod interval {
        use super::*;
        use crate::prelude::*;

        #[derive(Debug)]
        struct IntervalActor {
            running: Arc<AtomicBool>,
        }

        impl Actor for IntervalActor {
            async fn stopped(&mut self, _: &mut Context<Self>) {
                self.running.store(false, Ordering::SeqCst);
            }
        }

        impl Handler<()> for IntervalActor {
            async fn handle(&mut self, _: &mut Context<Self>, _cmd: ()) {
                self.running.store(true, Ordering::SeqCst);
            }
        }

        #[tokio::test]
        async fn stopped_when_actor_stopped() {
            let flag = Arc::new(AtomicBool::new(false));
            let addr = IntervalActor {
                running: Arc::clone(&flag),
            }
            .spawn();
            sleep(Duration::from_millis(300)).await;
            addr.halt().await.unwrap();
            sleep(Duration::from_millis(300)).await;
            assert!(
                !flag.load(Ordering::SeqCst),
                "Handler should not be called after actor is stopped"
            );
        }
    }

    mod interval_order {
        use std::sync::Mutex;
        use std::sync::atomic::AtomicU32;

        use super::*;
        use crate::{context::ContextID, prelude::*};

        use std::sync::LazyLock;

        static ATOMIC_VEC: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        fn append_to_log(item: impl Into<String>) {
            let vec = &*ATOMIC_VEC;
            vec.lock().unwrap().push(item.into());
        }

        fn print_log() -> usize {
            let vec = &*ATOMIC_VEC.lock().unwrap();
            for (i, line) in vec.iter().enumerate() {
                eprintln!("Log {i:?}: {line:?}");
            }
            vec.len()
        }

        #[derive(Debug)]
        struct IntervalActor {
            started: Instant,
            interval: Duration,
        }

        #[derive(Clone, Copy, Debug, Default)]
        struct IntervalSleep(Duration, u32);

        impl Message for IntervalSleep {
            type Response = ();
        }

        struct StopTasks;
        impl Message for StopTasks {
            type Response = ();
        }

        impl Handler<IntervalSleep> for IntervalActor {
            async fn handle(
                &mut self,
                _ctx: &mut Context<Self>,
                IntervalSleep(duration, invocation): IntervalSleep,
            ) {
                let call_id = ContextID::default();

                let called = Instant::now();
                let called_after_ms = called.duration_since(self.started).as_millis();
                append_to_log(format!(
                    "handle {call_id}/{invocation} called {called_after_ms}ms"
                ));

                sleep(duration).await;

                let woke_up = Instant::now();
                let woke_after_ms = woke_up.duration_since(self.started).as_millis();
                append_to_log(format!(
                    "handler {call_id}/{invocation} wake up {woke_after_ms}ms"
                ));
            }
        }

        impl Handler<StopTasks> for IntervalActor {
            async fn handle(&mut self, ctx: &mut Context<Self>, _: StopTasks) {
                append_to_log("stopping tasks");
                ctx.stop_tasks();
            }
        }

        impl Actor for IntervalActor {
            async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
                let invokation_id = AtomicU32::new(1);
                ctx.delayed_send(
                    || {
                        append_to_log("stopping tasks");
                        StopTasks
                    },
                    Duration::from_millis(80),
                );
                ctx.interval_with(
                    move || {
                        let invocation = invokation_id.fetch_add(1, Ordering::SeqCst);
                        append_to_log(format!("invoked {}", invocation));
                        IntervalSleep(Duration::from_millis(500), invocation)
                    },
                    self.interval,
                );
                Ok(())
            }
            async fn stopped(&mut self, _: &mut Context<Self>) {
                append_to_log("stopped");
            }
        }

        #[tokio::test]
        async fn dont_overlap_when_tasks_take_too_long() {
            let addr = crate::build(IntervalActor {
                started: Instant::now(),
                interval: Duration::from_millis(30),
            })
            .bounded(1)
            // .unbounded()
            .spawn();

            sleep(Duration::from_millis(600)).await;

            addr.halt().await.unwrap();
            let log_length = print_log();
            assert_eq!(log_length, 12);
        }
    }

    mod interval_with {
        use super::*;
        use crate::prelude::*;

        #[derive(Debug)]
        struct IntervalWithActor {
            running: Arc<AtomicBool>,
        }

        impl Actor for IntervalWithActor {
            async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
                ctx.interval_with(|| (), Duration::from_millis(100));
                Ok(())
            }
            async fn stopped(&mut self, _: &mut Context<Self>) {
                self.running.store(false, Ordering::SeqCst);
            }
        }

        impl Handler<()> for IntervalWithActor {
            async fn handle(&mut self, _: &mut Context<Self>, _: ()) {
                self.running.store(true, Ordering::SeqCst);
            }
        }

        #[tokio::test]
        async fn stopped_when_actor_stopped() {
            let running = Arc::new(AtomicBool::new(false));
            let addr = IntervalWithActor {
                running: Arc::clone(&running),
            }
            .spawn();
            sleep(Duration::from_millis(300)).await;
            addr.halt().await.unwrap();
            sleep(Duration::from_millis(300)).await;
            assert!(
                !running.load(Ordering::SeqCst),
                "Handler should not be called after actor is stopped"
            );
        }
    }

    mod delayed_send {
        use super::*;
        use crate::prelude::*;

        #[derive(Debug)]
        struct DelayedSendActor {
            running: Arc<AtomicBool>,
        }

        impl Actor for DelayedSendActor {
            async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
                ctx.delayed_send(|| (), Duration::from_millis(100));
                Ok(())
            }
            async fn stopped(&mut self, _: &mut Context<Self>) {
                self.running.store(false, Ordering::SeqCst);
            }
        }

        impl Handler<()> for DelayedSendActor {
            async fn handle(&mut self, _: &mut Context<Self>, _: ()) {
                self.running.store(true, Ordering::SeqCst);
            }
        }

        #[tokio::test]
        async fn stopped_when_actor_stopped() {
            let running = Arc::new(AtomicBool::new(false));
            let addr = DelayedSendActor {
                running: Arc::clone(&running),
            }
            .spawn();
            sleep(Duration::from_millis(300)).await;
            addr.halt().await.unwrap();
            sleep(Duration::from_millis(300)).await;
            assert!(
                !running.load(Ordering::SeqCst),
                "Handler should not be called after actor is stopped"
            );
        }
    }
}
