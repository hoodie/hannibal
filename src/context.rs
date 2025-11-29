use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use futures::channel::oneshot;

use crate::{
    Addr, Handler, Message, RestartableActor, Sender, WeakAddr,
    actor::Actor,
    channel::WeakTx,
    context::task_id::TaskID,
    error::{ActorError::AlreadyStopped, Result},
    event_loop::Payload,
    runtime,
};
pub use context_id::ContextID;

pub type RunningFuture = futures::future::Shared<oneshot::Receiver<()>>;
pub struct StopNotifier(pub(crate) oneshot::Sender<()>);
impl StopNotifier {
    pub fn notify(self) {
        self.0.send(()).ok();
    }
}

mod context_id {
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

mod task_id {
    use std::sync::{LazyLock, atomic::AtomicU64};

    static TASK_ID: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TaskID(u64);

    impl Default for TaskID {
        fn default() -> Self {
            Self(TASK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        }
    }

    #[test]
    fn ids_go_up() {
        let id1 = TaskID::default();
        let id2 = TaskID::default();
        let id3 = TaskID::default();
        assert!(id1 < id2);
        assert!(id2 < id3);
    }
}

type AnyBox = Box<dyn Any + Send + Sync>;

/// Available to the actor in every execution call.
///
/// The context is used to interact with the actor system.
/// You can start intervals, send messages to yourself, and stop the actor.
///
pub struct Context<A> {
    pub(crate) id: ContextID,
    pub(crate) weak_tx: WeakTx<A>,
    pub(crate) running: RunningFuture,
    pub(crate) children: HashMap<TypeId, Vec<AnyBox>>,
    // TODO: make this a slab and use unique ids to address handles so that users can actually stop intervals again
    pub(crate) tasks: HashMap<TaskID, futures::future::AbortHandle>,
}

impl<A> Drop for Context<A> {
    fn drop(&mut self) {
        for (_, task) in self.tasks.drain() {
            task.abort();
        }
    }
}

/// ## Life-cycle
impl<A: Actor> Context<A> {
    /// Stop the actor.
    pub fn stop(&self) -> Result<()> {
        if let Some(sender) = self.weak_tx.upgrade() {
            Ok(sender.force_send(Payload::Stop)?)
        } else {
            Err(AlreadyStopped)
        }
    }
}

/// ## Child Actors
impl<A: Actor> Context<A> {
    /// Add a child actor.
    ///
    /// This child actor is held until this context is stopped.
    pub fn add_child(&mut self, child: impl Into<Sender<()>>) {
        self.children
            .entry(TypeId::of::<()>())
            .or_default()
            .push(Box::new(child.into()));
    }

    /// Register a child actor by `Message` type.
    ///
    /// This actor will be held until this actor is stopped via a `Sender`.
    ///
    pub fn register_child<M: Message<Response = ()>>(&mut self, child: impl Into<Sender<M>>) {
        self.children
            .entry(TypeId::of::<M>())
            .or_default()
            .push(Box::new(child.into()));
    }

    /// Send a message to all child actors registered with this message type.
    pub fn send_to_children<M>(&mut self, message: M)
    where
        M: Message<Response = ()>,
        M: Clone,
    {
        let key = TypeId::of::<M>();

        if let Some(children) = self.children.get(&key) {
            for child in children
                .iter()
                .filter_map(|child| child.downcast_ref::<Sender<M>>())
            {
                if let Err(error) = child.force_send(message.clone()) {
                    log::error!("Failed to send message to child: {error}");
                }
            }
        }
    }
}

/// ## Creating `Addr`s, `Caller`s and `Sender`s to yourself
impl<A: Actor> Context<A> {
    /// Create an weak address to the actor.
    pub fn weak_address(&self) -> WeakAddr<A> {
        let weak_tx = self.weak_tx.clone();

        let context_id = self.id;
        let running = self.running.clone();
        let running_inner = self.running.clone();
        let upgrade = Box::new(move || {
            let running = running_inner.clone();
            weak_tx.upgrade().map(|tx| Addr {
                context_id,
                tx,
                running,
            })
        });

        WeakAddr::new(context_id, upgrade, running)
    }

    /// Create a weak sender to the actor.
    pub fn weak_sender<M: crate::Message<Response = ()>>(&self) -> crate::WeakSender<M>
    where
        A: Handler<M>,
    {
        crate::WeakSender::from_weak_tx(self.weak_tx.clone(), self.id)
    }

    /// Create a weak caller to the actor.
    pub fn weak_caller<M: crate::Message<Response = R>, R>(&self) -> crate::WeakCaller<M>
    where
        A: Handler<M>,
    {
        crate::WeakCaller::from_weak_tx(self.weak_tx.clone(), self.id)
    }
}

/// ## Broker Interaction
impl<A: Actor> Context<A> {
    /// Publish to the broker.
    ///
    /// Every actor can publish messages to the broker
    /// which will be delivered to all actors that subscribe to the message.
    pub async fn publish<M: crate::Message<Response = ()> + Clone>(&self, message: M) -> Result<()>
    where
        A: Handler<M>,
    {
        crate::Broker::publish(message).await
    }

    /// Subscribe to a message.
    ///
    /// The actor will receive all messages of this type.
    pub async fn subscribe<M: crate::Message<Response = ()> + Clone>(&mut self) -> Result<()>
    where
        A: Handler<M>,
    {
        crate::Broker::subscribe(self.weak_sender()).await
    }
}

use futures::FutureExt;
use std::{future::Future, time::Duration};

/// Represents a handle to a task that can be used to abort the task.
#[derive(Copy, Clone)]
pub struct TaskHandle(TaskID);

impl std::fmt::Debug for TaskHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TaskHandle").finish()
    }
}

async fn interval_timeout<T>(fut: impl Future<Output = T>, timeout: Duration) -> Option<T> {
    futures::select_biased! {
        success = fut.fuse() => Some(success),
        _ = FutureExt::fuse(crate::runtime::sleep(timeout)) => None
    }
}

/// ## Task Handling
impl<A: Actor> Context<A> {
    /// Spawn a task that will be executed in the background.
    ///
    /// The task will be aborted when the actor is stopped.
    pub fn spawn_task(&mut self, task: impl Future<Output = ()> + Send + 'static) -> TaskHandle {
        let (task, handle) = futures::future::abortable(task);

        let task_id = TaskHandle(TaskID::default());
        self.tasks.insert(task_id.0, handle);
        #[cfg(feature = "tokio_runtime")]
        tokio::spawn(task.map(|_| ()));
        #[cfg(not(feature = "tokio_runtime"))]
        async_global_executor::spawn(task.map(|_| ())).detach();
        task_id
    }

    /// Send yourself a message at a regular interval.
    ///
    /// ## Backpressure
    ///
    /// Ticks that take longer than 50% of the interval duration to send are skipped
    /// to prevent stale message buildup when the actor cannot keep pace with the interval.
    pub fn interval<M: Message<Response = ()> + Clone + Send + 'static>(
        &mut self,
        message: M,
        duration: Duration,
    ) -> TaskHandle
    where
        A: Handler<M> + Send + 'static,
    {
        let myself = self.weak_sender();
        self.spawn_task(async move {
            let expiry = duration
                .mul_f32(0.5)
                .clamp(Duration::from_millis(100), Duration::from_secs(5));
            log::trace!("Starting interval with message");
            loop {
                runtime::sleep(duration).await;
                let Some(sender) = myself.upgrade() else {
                    log::trace!("interval ended after actor dropped");
                    break;
                };
                log::trace!("sending interval msg after sleep");
                let Some(sent) = interval_timeout(sender.send(message.clone()), expiry).await
                else {
                    log::warn!("tick missed by {}ms, skipping", expiry.as_millis());
                    continue;
                };
                if let Err(error) = sent {
                    log::warn!("interval message failed: {error}");
                    break;
                }
            }
            log::trace!("Interval stopped");
        })
    }

    /// Send yourself a message at a regular interval.
    ///
    /// ## Backpressure
    ///
    /// Ticks that take longer than 50% of the interval duration to send are skipped
    /// to prevent stale message buildup when the actor cannot keep pace with the interval.
    ///
    /// Warning: don't do anything expensive in the message function, as it will block the interval.
    pub fn interval_with<M: Message<Response = ()>>(
        &mut self,
        message_fn: impl Fn() -> M + Send + Sync + 'static,
        duration: Duration,
    ) -> TaskHandle
    where
        A: Handler<M>,
    {
        let myself = self.weak_sender();
        self.spawn_task(async move {
            let expiry = duration
                .mul_f32(0.5)
                .clamp(Duration::from_millis(100), Duration::from_secs(5));
            log::trace!("Starting interval with message_fn");
            loop {
                runtime::sleep(duration).await;
                let Some(sender) = myself.upgrade() else {
                    log::trace!("interval ended after actor dropped");
                    break;
                };
                log::trace!("sending interval msg after sleep");
                let Some(sent) = interval_timeout(sender.send(message_fn()), expiry).await else {
                    log::warn!("tick missed by {}ms, skipping", expiry.as_millis());
                    continue;
                };
                if let Err(error) = sent {
                    log::warn!("interval message failed: {error}");
                    break;
                }
            }
            log::trace!("Interval stopped");
        })
    }

    /// Send yourself a message after a delay.
    pub fn delayed_send<M: Message<Response = ()>>(
        &mut self,
        message_fn: impl Fn() -> M + Send + Sync + 'static,
        duration: Duration,
    ) -> TaskHandle
    where
        A: Handler<M>,
    {
        let myself = self.weak_sender();
        self.spawn_task(async move {
            log::trace!("Scheduling delayed send");
            runtime::sleep(duration).await;

            if myself.upgrade_and_send(message_fn()).await.is_err() {
                log::warn!("Failed to send message");
            }
            log::trace!("Delayed send completed");
        })
    }

    /// Execute a task after a delay.
    pub fn delayed_exec<F: Future<Output = ()> + Send + 'static>(
        &mut self,
        task: F,
        duration: Duration,
    ) -> TaskHandle {
        self.spawn_task(async move {
            runtime::sleep(duration).await;
            task.await;
        })
    }

    /// Stop a very specific task.
    pub fn stop_task(&mut self, handle: TaskHandle) {
        if let Some((_, task)) = self.tasks.remove_entry(&handle.0) {
            task.abort();
        }
    }
}

/// Life-cycle
impl<A: RestartableActor> Context<A> {
    /// Restart the actor.
    ///
    /// Depending on the `RestartStrategy` of the particular [`Actor`] this will do either of two things:
    ///
    /// *`RestartOnly`*: call the [`Actor::stopped()`] and [`Actor::started()`] methods in order
    /// *`RecreateFromDefault`*: create a new instance of the actor and start it.
    ///
    pub fn restart(&self) -> Result<()> {
        if let Some(sender) = self.weak_tx.upgrade() {
            Ok(sender.force_send(Payload::Restart)?)
        } else {
            Err(AlreadyStopped)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod interval_cleanup {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use crate::runtime::sleep;

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
        use super::*;
        use crate::prelude::*;

        #[test_log::test(tokio::test)]
        async fn handlers_never_overlap() {
            use std::sync::Arc;
            use std::sync::atomic::{AtomicBool, AtomicU32};

            let handler_running = Arc::new(AtomicBool::new(false));
            let overlap_detected = Arc::new(AtomicBool::new(false));
            let handler_count = Arc::new(AtomicU32::new(0));

            struct NoOverlapActor {
                handler_running: Arc<AtomicBool>,
                overlap_detected: Arc<AtomicBool>,
                handler_count: Arc<AtomicU32>,
            }

            #[derive(Clone)]
            #[message]
            struct SlowMessage;

            impl Handler<SlowMessage> for NoOverlapActor {
                async fn handle(&mut self, _ctx: &mut Context<Self>, _: SlowMessage) {
                    let was_running = self.handler_running.swap(true, Ordering::SeqCst);

                    if was_running {
                        self.overlap_detected.store(true, Ordering::SeqCst);
                        panic!(
                            "Handler overlap detected! A handler started while another was still running."
                        );
                    }

                    self.handler_count.fetch_add(1, Ordering::SeqCst);

                    sleep(Duration::from_millis(100)).await;

                    self.handler_running.store(false, Ordering::SeqCst);
                }
            }

            impl Actor for NoOverlapActor {
                async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
                    ctx.interval_with(|| SlowMessage, Duration::from_millis(20));
                    Ok(())
                }
            }

            let addr = crate::build(NoOverlapActor {
                handler_running: Arc::clone(&handler_running),
                overlap_detected: Arc::clone(&overlap_detected),
                handler_count: Arc::clone(&handler_count),
            })
            .bounded(10)
            .spawn();

            sleep(Duration::from_millis(300)).await;
            addr.halt().await.unwrap();

            assert!(
                !overlap_detected.load(Ordering::SeqCst),
                "Handlers should never overlap"
            );

            let count = handler_count.load(Ordering::SeqCst);
            assert!(
                count >= 2,
                "At least 2 handlers should have executed (got {count})",
            );

            assert!(
                !handler_running.load(Ordering::SeqCst),
                "No handler should be running after halt"
            );
        }
    }

    mod interval_with {
        use super::*;
        use crate::{TaskHandle, actor::spawnable::Spawnable, prelude::*};

        #[derive(Debug)]
        struct IntervalWithActor {
            running: Arc<AtomicBool>,
            interval: Option<TaskHandle>,
        }

        impl Actor for IntervalWithActor {
            async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
                self.interval
                    .replace(ctx.interval_with(|| (), Duration::from_millis(100)));
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
        async fn stopped_by_task_handle() {
            let running = Arc::new(AtomicBool::new(false));
            let addr = IntervalWithActor {
                running: Arc::clone(&running),
                interval: None,
            }
            .spawn();
            sleep(Duration::from_millis(300)).await;

            #[derive(hannibal_derive::Message)]
            struct StopInterval;
            impl Handler<StopInterval> for IntervalWithActor {
                async fn handle(
                    &mut self,
                    ctx: &mut Context<Self>,
                    _msg: StopInterval,
                ) -> <StopInterval as Message>::Response {
                    self.running.store(false, Ordering::SeqCst);
                    if let Some(interval) = self.interval {
                        ctx.stop_task(interval)
                    }
                }
            }

            addr.send(StopInterval).await.unwrap();
            sleep(Duration::from_millis(300)).await;
            assert!(
                !running.load(Ordering::SeqCst),
                "Handler should not be called after actor is stopped"
            );
        }

        #[tokio::test]
        async fn stopped_when_actor_stopped() {
            let running = Arc::new(AtomicBool::new(false));
            let addr = IntervalWithActor {
                running: Arc::clone(&running),
                interval: None,
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
