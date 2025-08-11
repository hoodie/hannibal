use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use futures::channel::oneshot;

use crate::{
    Addr, Handler, Message, RestartableActor, Sender, WeakAddr,
    actor::Actor,
    channel::{WeakChanTx, WeakForceChanTx},
    context::task_id::TaskID,
    environment::Payload,
    error::{ActorError::AlreadyStopped, Result},
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
    #[allow(dead_code)]
    pub(crate) weak_tx: WeakChanTx<A>,
    pub(crate) weak_force_tx: WeakForceChanTx<A>,
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
        if let Some(tx) = self.weak_force_tx.upgrade() {
            Ok(tx.send(Payload::Stop)?)
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

/// ## Creating `Addr`s, `Caller`s and `Sender`s to yoursef
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

    /// Create a weak caller to the actor.
    pub fn weak_caller<M: crate::Message<Response = R>, R>(&self) -> crate::WeakCaller<M>
    where
        A: Handler<M>,
    {
        crate::WeakCaller::from_weak_tx(std::sync::Weak::clone(&self.weak_tx), self.id)
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

/// ## Task Handling
impl<A: Actor> Context<A> {
    /// Spawn a task that will be executed in the background.
    ///
    /// The task will be aborted when the actor is stopped.
    pub fn spawn_task(&mut self, task: impl Future<Output = ()> + Send + 'static) -> TaskHandle {
        let (task, handle) = futures::future::abortable(task);

        let task_id = TaskHandle(TaskID::default());
        self.tasks.insert(task_id.0, handle);
        async_global_executor::spawn(task.map(|_| ())).detach();
        task_id
    }

    #[cfg(test)]
    pub(crate) fn stop_tasks(&mut self) {
        for (_, handle) in self.tasks.drain() {
            handle.abort();
        }
    }

    /// Send yourself a message at a regular interval.
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
            loop {
                sleep(duration).await;
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
    ) -> TaskHandle
    where
        A: Handler<M>,
    {
        let myself = self.weak_sender();
        self.spawn_task(async move {
            loop {
                runtime::sleep(duration).await;
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
    ) -> TaskHandle
    where
        A: Handler<M>,
    {
        let myself = self.weak_sender();
        self.spawn_task(async move {
            runtime::sleep(duration).await;

            if myself.try_send(message_fn()).await.is_err() {
                log::warn!("Failed to send message");
            }
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
    pub fn restart(&self) -> Result<()> {
        if let Some(tx) = self.weak_force_tx.upgrade() {
            Ok(tx.send(Payload::Restart)?)
        } else {
            Err(AlreadyStopped)
        }
    }
}

#[cfg(test)]
mod interval_cleanup {
    #![allow(clippy::unwrap_used)]

    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
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
                let invocation_id = AtomicU32::new(1);
                ctx.delayed_send(
                    || {
                        append_to_log("stopping tasks");
                        StopTasks
                    },
                    Duration::from_millis(80),
                );
                ctx.interval_with(
                    move || {
                        let invocation = invocation_id.fetch_add(1, Ordering::SeqCst);
                        append_to_log(format!("invoked {invocation}"));
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
        use crate::{actor::spawnable::Spawnable, context::task_handling::TaskHandle, prelude::*};

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

            // #[derive(hannibal_derive::Message)]
            struct StopInterval;
            impl Message for StopInterval {
                type Response = ();
            }
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
