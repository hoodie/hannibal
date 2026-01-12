use std::collections::HashMap;

use futures::channel::oneshot;

use crate::{
    Handler, Message, RestartableActor, WeakAddr,
    actor::Actor,
    channel::WeakTx,
    error::{
        ActorError::{self, ActorDropped, AlreadyStopped},
        Result,
    },
    event_loop::Payload,
    runtime,
};

mod context_id;
mod core;
mod task_id;

#[cfg(test)]
mod test_interval_cleanup;

pub(crate) use self::{context_id::ContextID, core::Core, task_id::TaskID};

// Runtime-specific task handle type
#[cfg(all(feature = "tokio_runtime", not(feature = "async_runtime")))]
type TaskJoinHandle = tokio::task::JoinHandle<()>;

#[cfg(all(not(feature = "tokio_runtime"), feature = "async_runtime"))]
type TaskJoinHandle = async_global_executor::Task<()>;

pub type RunningFuture = futures::future::Shared<oneshot::Receiver<()>>;
pub struct StopNotifier(pub(crate) oneshot::Sender<()>);
impl StopNotifier {
    pub fn notify(self) {
        self.0.send(()).ok();
    }
}

/// Available to the actor in every execution call.
///
/// The context is used to interact with the actor system.
/// You can start intervals, send messages to yourself, and stop the actor.
///
pub struct Context<A> {
    pub(crate) core: Core,
    pub(crate) weak_tx: WeakTx<A>,
    pub(crate) children: children::Children,
    // TODO: consider using a tokio::joinset/ futures::FuturesUnordered
    pub(crate) tasks: HashMap<TaskID, TaskJoinHandle>,
}

impl<A> Drop for Context<A> {
    fn drop(&mut self) {
        for (_, task) in self.tasks.drain() {
            #[cfg(feature = "tokio_runtime")]
            task.abort();

            // For async_runtime, dropping the Task handle cancels it automatically
            #[cfg(not(feature = "tokio_runtime"))]
            drop(task);
        }
    }
}

/// ## Life-cycle
impl<A: Actor> Context<A> {
    /// Stop the actor.
    pub fn stop(&self) -> Result<()> {
        if let Some(tx) = self.weak_tx.upgrade() {
            tx.force_send(Payload::Stop).map_err(|_| AlreadyStopped)?;
            Ok(())
        } else {
            Err(ActorDropped)
        }
    }
}

/// ## Child Actors
impl<A: Actor> Context<A> {
    /// Add a child actor.
    ///
    /// This child actor is held until this context is stopped.
    pub fn add_child(&mut self, child: impl Into<Sender<()>>) {
        self.children.add(child)
    }

    /// Register a child actor by `Message` type.
    ///
    /// This actor will be held until this actor is stopped via a `Sender`.
    ///
    pub fn register_child<M: Message<Response = ()>>(&mut self, child: impl Into<Sender<M>>) {
        self.children.register(child)
    }

    /// Send a message to all child actors registered with this message type.
    pub fn send_to_children<M: Message<Response = ()> + Clone>(&mut self, message: M) {
        self.children.forward(message);
    }

    /// Perform context-local garbage collection.
    ///
    /// This method:
    ///
    /// - Removes child actors that have fully stopped.
    /// - Drops join handles for background tasks that have already finished.
    ///
    /// Running tasks and live child actors are **not** affected: this method does
    /// not cancel or stop anything that is still in progress. It only cleans up
    /// bookkeeping for work that has already completed.
    ///
    /// # When to call this
    ///
    /// `gc` is **not** called automatically by the runtime. If your actor spawns
    /// many short‑lived tasks or children, you should call `gc` periodically to
    /// release their resources from the `Context`. A common pattern is to call it:
    ///
    /// - At the end of a message handler that may have spawned new tasks.
    /// - On a timer or in response to a "maintenance" message.
    ///
    /// For actors that rarely spawn tasks or children, calling `gc` occasionally
    /// (or not at all) may be sufficient.
    ///
    /// # Performance
    ///
    /// `gc` iterates over all tracked tasks and children to remove completed ones.
    /// The cost is roughly proportional to the number of entries being tracked.
    /// It is typically inexpensive for a modest number of tasks, but if your actor
    /// tracks many thousands of tasks you may want to adjust how often you call
    /// `gc` to balance cleanup latency against overhead.
    ///
    /// # Example
    ///
    /// ```ignore
    /// impl Handler<MyMessage> for MyActor {
    ///     type Response = ();
    ///
    ///     fn handle(&mut self, msg: MyMessage, ctx: &mut Context<Self>) {
    ///         // Potentially spawn a new background task or child here...
    ///
    ///         // Periodically clean up finished tasks and stopped children.
    ///         ctx.gc();
    ///     }
    /// }
    /// ```
    pub fn gc(&mut self) {
        self.children.remove_stopped();
        let initial_count = self.tasks.len();
        self.tasks = self
            .tasks
            .drain()
            .filter(|(_id, handle)| !handle.is_finished())
            .collect();
        let removed_count = initial_count - self.tasks.len();
        if removed_count > 0 {
            log::trace!(
                "gc: removed {} finished task(s), {} remaining",
                removed_count,
                self.tasks.len()
            );
        }
    }
}

mod children {
    use std::{any::TypeId, collections::HashMap};

    use crate::{Message, Sender};

    /// Trait to check if something is still alive/running.
    ///
    /// This allows checking liveness of actors or senders regardless of their concrete type.
    pub(crate) trait IsAlive: Send + Sync {
        fn is_alive(&self) -> bool;
        fn as_any(&self) -> &dyn std::any::Any;
    }

    impl<M: Message<Response = ()>> IsAlive for Sender<M> {
        fn is_alive(&self) -> bool {
            self.running()
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    type Child = Box<dyn IsAlive>;

    #[derive(Default)]
    pub struct Children {
        pub(crate) children: HashMap<TypeId, Vec<Child>>,
    }
    impl Children {
        pub fn add(&mut self, child: impl Into<Sender<()>>) {
            self.children
                .entry(TypeId::of::<()>())
                .or_default()
                .push(Box::new(child.into()));
        }
        pub fn register<M: Message<Response = ()>>(&mut self, child: impl Into<Sender<M>>) {
            self.children
                .entry(TypeId::of::<M>())
                .or_default()
                .push(Box::new(child.into()));
        }

        pub fn remove_stopped(&mut self) {
            for children in self.children.values_mut() {
                let initial_count = children.len();
                children.retain(|child| child.is_alive());
                let removed_count = initial_count - children.len();
                if removed_count > 0 {
                    log::trace!(
                        "gc: removed {} stopped child(ren), {} remaining",
                        removed_count,
                        children.len()
                    );
                }
            }
        }

        /// Send a message to all child actors registered with this message type.
        pub fn forward<M>(&mut self, message: M)
        where
            M: Message<Response = ()>,
            M: Clone,
        {
            let key = TypeId::of::<M>();

            if let Some(children) = self.children.get(&key) {
                for child in children
                    .iter()
                    .filter_map(|child| child.as_any().downcast_ref::<Sender<M>>())
                {
                    // TODO: force_send is not correct here, we should have a try_send mechanism instead
                    if let Err(error) = child.force_send(message.clone()) {
                        log::error!("Failed to send message to child: {error}");
                    }
                }
            }
        }
    }
}

/// ## Creating `Addr`s, `Caller`s and `Sender`s to yourself
impl<A: Actor> Context<A> {
    /// Create a weak address to the actor.
    pub fn weak_address(&self) -> WeakAddr<A> {
        WeakAddr::new(self.core.clone(), self.weak_tx.clone())
    }

    /// Create a weak sender to the actor.
    pub fn weak_sender<M: crate::Message<Response = ()>>(&self) -> crate::WeakSender<M>
    where
        A: Handler<M>,
    {
        crate::WeakSender::from_weak_tx(self.weak_tx.clone(), self.core.clone())
    }

    /// Create a weak caller to the actor.
    pub fn weak_caller<M: crate::Message<Response = R>, R>(&self) -> crate::WeakCaller<M>
    where
        A: Handler<M>,
    {
        crate::WeakCaller::from_weak_tx(self.weak_tx.clone(), self.core.clone())
    }
}

/// ## Broker Interaction
impl<A: Actor> Context<A> {
    /// Publish to the broker.
    ///
    /// Every actor can publish messages to the broker
    /// which will be delivered to all actors that subscribe to the message.
    pub async fn publish<M: crate::Message<Response = ()> + Clone>(
        &self,
        message: M,
    ) -> Result<()> {
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

use super::addr::sender::Sender;

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
    ///
    /// Returns a [`TaskHandle`] that can be used to check if the task is finished
    /// or to stop it manually using [`Context::stop_task`].
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// impl Handler<StartWork> for MyActor {
    ///     async fn handle(&mut self, ctx: &mut Context<Self>, _: StartWork) {
    ///         let handle = ctx.spawn_task(async {
    ///             // Long-running background work
    ///             do_work().await;
    ///         });
    ///
    ///         // Check if task is still running
    ///         if let Some(false) = ctx.is_task_finished(&handle) {
    ///             println!("Task is still running");
    ///         }
    ///
    ///         // Optionally stop it later
    ///         ctx.stop_task(handle);
    ///     }
    /// }
    /// ```
    pub fn spawn_task(&mut self, task: impl Future<Output = ()> + Send + 'static) -> TaskHandle {
        let task_id = TaskHandle(TaskID::default());

        #[cfg(feature = "tokio_runtime")]
        let handle = tokio::spawn(task);

        #[cfg(not(feature = "tokio_runtime"))]
        let handle = async_global_executor::spawn(task);

        self.tasks.insert(task_id.0, handle);

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

    /// Check if a task is finished.
    ///
    /// Returns `None` if the task handle is invalid (task was already removed).
    pub fn is_task_finished(&self, handle: &TaskHandle) -> Option<bool> {
        let task = self.tasks.get(&handle.0)?;

        #[cfg(feature = "tokio_runtime")]
        {
            Some(task.is_finished())
        }
        #[cfg(not(feature = "tokio_runtime"))]
        {
            Some(task.is_finished())
        }
    }

    /// Stop a specific task by aborting it and removing it from the task list.
    pub fn stop_task(&mut self, handle: TaskHandle) {
        if let Some(task) = self.tasks.remove(&handle.0) {
            #[cfg(feature = "tokio_runtime")]
            task.abort();

            // For async_runtime, dropping the Task handle cancels it automatically
            #[cfg(feature = "async_runtime")]
            drop(task);
        }
    }
}

/// Life-cycle
impl<A: RestartableActor> Context<A> {
    /// Restart the actor.
    ///
    /// The behavior depends on the restart strategy configured via the builder:
    ///
    /// ## `RestartOnly` (default)
    ///
    /// Calls [`Actor::stopped()`] then [`Actor::started()`] on the **same instance**.
    /// The actor's state is preserved—only the lifecycle hooks are re-triggered.
    /// This is the default when using [`hannibal::setup_actor()`](crate::build).
    ///
    /// ## `RecreateFromDefault`
    ///
    /// Calls [`Actor::stopped()`], creates a **new instance** via `Default::default()`,
    /// then calls [`Actor::started()`]. All previous state is discarded.
    /// Enable this with [`ActorBuilder::recreate_from_default()`](crate::builder::ActorBuilder::recreate_from_default).
    ///
    /// ## Note: Stream Actors
    ///
    /// Actors spawned with [`ActorBuilder::on_stream()`](crate::builder::ActorBuilder::on_stream)
    /// cannot be restarted, as streams cannot be replayed.
    ///
    pub fn restart(&self) -> Result<()> {
        if let Some(tx) = self.weak_tx.upgrade() {
            tx.force_send(Payload::Restart)
                .map_err(|_err| ActorError::AlreadyStopped)?;
            Ok(())
        } else {
            Err(ActorDropped)
        }
    }
}
