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
                runtime::sleep(duration).await;
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
    ///
    /// Depending on the `RestartStrategy` of the particular [`Actor`] this will do either of two things:
    ///
    /// *`RestartOnly`*: call the [`Actor::stopped()`] and [`Actor::started()`] methods in order
    /// *`RecreateFromDefault`*: create a new instance of the actor and start it.
    ///
    pub fn restart(&self) -> Result<()> {
        if let Some(tx) = self.weak_force_tx.upgrade() {
            Ok(tx.send(Payload::Restart)?)
        } else {
            Err(AlreadyStopped)
        }
    }
}

#[cfg(test)]
mod test_log {
    #![allow(clippy::unwrap_used)]
    use std::{
        sync::{LazyLock, Mutex, OnceLock},
        time::Instant,
    };

    pub static ATOMIC_VEC: LazyLock<Mutex<Vec<(String, usize)>>> = LazyLock::new(Default::default);
    pub static STARTED: OnceLock<Instant> = OnceLock::new();

    pub fn append_to_log(item: impl Into<String>, kind: impl Into<usize>) {
        #[cfg(feature = "tokio_runtime")]
        let task_id = tokio::task::try_id();
        let started = STARTED.get_or_init(Instant::now);
        let elapsed = started.elapsed().as_millis();
        let msg = item.into();
        #[cfg(feature = "tokio_runtime")]
        let log_line = format!("[{:>5} ms] {:?} {}", elapsed, task_id, msg);
        #[cfg(not(feature = "tokio_runtime"))]
        let log_line = format!("[{:>5} ms] {}", elapsed, msg);
        let vec = &*ATOMIC_VEC;
        vec.lock().unwrap().push((log_line, kind.into()));
    }

    pub fn log_events() -> Vec<usize> {
        let vec = &*ATOMIC_VEC.lock().unwrap();
        vec.iter().map(|(_, evt)| *evt).collect()
    }

    pub fn print_log() -> usize {
        let vec = &*ATOMIC_VEC.lock().unwrap();
        for (i, (line, evt)) in vec.iter().enumerate() {
            eprintln!("Log {:>3}: {evt} {line:?}", i + 1);
        }
        vec.len()
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
        use std::{sync::atomic::AtomicU32, time::Instant};
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        enum EventKind {
            HandleSleep = 0,
            HandleSleepPostSleep = 1,
            HandleStop = 2,
            SendDelay = 3,
            SendInterval = 4,
            TriggerHalt = 5,
            Stopped = 6,
        }

        impl From<usize> for EventKind {
            fn from(val: usize) -> Self {
                match val {
                    0 => EventKind::HandleSleep,
                    1 => EventKind::HandleSleepPostSleep,
                    2 => EventKind::HandleStop,
                    3 => EventKind::SendDelay,
                    4 => EventKind::SendInterval,
                    5 => EventKind::TriggerHalt,
                    6 => EventKind::Stopped,
                    _ => panic!("Invalid event kind"),
                }
            }
        }

        impl From<EventKind> for usize {
            fn from(val: EventKind) -> Self {
                val as usize
            }
        }

        use super::*;
        use crate::{
            context::{
                ContextID,
                test_log::{STARTED, append_to_log, log_events, print_log},
            },
            prelude::*,
        };

        #[derive(Debug)]
        struct IntervalActor {
            interval: Duration,
        }

        impl IntervalActor {
            // runtime() is no longer needed, so it can be removed
        }

        #[derive(Clone, Copy, Debug, Default)]
        struct IntervalSleep {
            duration: Duration,
            count: u32,
        }

        impl Message for IntervalSleep {
            type Response = ();
        }

        impl IntervalSleep {
            fn new(duration: Duration, count: u32) -> Self {
                Self { duration, count }
            }

            async fn sleep(&self) {
                sleep(self.duration).await;
            }
        }

        struct StopTasks;
        impl Message for StopTasks {
            type Response = ();
        }

        impl Handler<IntervalSleep> for IntervalActor {
            async fn handle(&mut self, _ctx: &mut Context<Self>, sleep_msg: IntervalSleep) {
                let call_id = ContextID::default();

                append_to_log(
                    format!("Handle<IntervalSleep> {call_id}/{} called", sleep_msg.count),
                    EventKind::HandleSleep,
                );

                sleep_msg.sleep().await;

                append_to_log(
                    format!(
                        "Handle<IntervalSleep> {call_id}/{} post sleep of {:?}",
                        sleep_msg.count, sleep_msg.duration
                    ),
                    EventKind::HandleSleepPostSleep,
                );
            }
        }

        impl Handler<StopTasks> for IntervalActor {
            async fn handle(&mut self, ctx: &mut Context<Self>, _: StopTasks) {
                append_to_log("handing StopTasks -> stopping tasks", EventKind::HandleStop);
                ctx.stop_tasks();
            }
        }

        impl Actor for IntervalActor {
            async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
                let invocation_id = AtomicU32::new(1);
                let stop_delay = 80;
                ctx.delayed_send(
                    move || {
                        append_to_log(
                            format!("sending: StopTasks after {stop_delay}ms"),
                            EventKind::SendDelay,
                        );
                        StopTasks
                    },
                    Duration::from_millis(stop_delay),
                );
                let interval = Duration::from_millis(200);
                let self_interval = self.interval;
                ctx.interval_with(
                    move || {
                        let invocation = invocation_id.fetch_add(1, Ordering::SeqCst);
                        append_to_log(
                            format!(
                                "sending: IntervalSleep {invocation} every {:?}",
                                self_interval
                            ),
                            EventKind::SendInterval,
                        );
                        IntervalSleep::new(interval, invocation)
                    },
                    self.interval,
                );
                Ok(())
            }
            async fn stopped(&mut self, _: &mut Context<Self>) {
                append_to_log("🏁 Actor stopped", EventKind::Stopped);
            }
        }

        #[tokio::test]
        async fn dont_overlap_when_tasks_take_too_long() {
            // Ensure STARTED is reset for each test run
            STARTED.set(Instant::now()).ok();

            let addr = crate::build(IntervalActor {
                interval: Duration::from_millis(70),
            })
            .bounded(1)
            // .unbounded()
            .spawn();

            let halt_after = Duration::from_millis(700);
            sleep(halt_after).await;
            append_to_log(
                format!("✋ HALTING ACTOR AFTER {halt_after:?}"),
                EventKind::TriggerHalt,
            );
            addr.halt().await.unwrap();
            print_log();
            use EventKind::*;
            assert_eq!(
                log_events()
                    .into_iter()
                    .map(EventKind::from)
                    .collect::<Vec<_>>(),
                [
                    SendInterval,
                    HandleSleep,
                    SendDelay,
                    SendInterval,
                    HandleSleepPostSleep,
                    HandleStop,
                    HandleSleep,
                    HandleSleepPostSleep,
                    TriggerHalt,
                    Stopped,
                ]
            )
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
