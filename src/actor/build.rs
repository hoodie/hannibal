use super::{Actor, builder, spawner};

/// Start building and configure you actor.
///
/// One feature of hannibal is that you can configure your actor's runtime behaving before spawning.
/// This includes
/// 1. What kind of channels do you use under the hood and how large are the channel's buffers?
/// 2. Should the actor create a fresh object on restart?
/// 3. Should it listen to a stream.
/// 4. Should the actor enfore timeouts when waiting?
/// 5. Register as a service
///
/// ## 1. What kind of channels do you use under the hood and how large are the channel's buffers?
/// Unbounded versus Bounded.
///
/// ### Example
/// You can configure what kind of channel the actor uses and what its capacity should be.
///
/// ```no_run
/// # #[derive(hannibal_derive::Actor,hannibal_derive::RestartableActor, Default)]
/// # struct MyActor;
/// // start MyActor with a mailbox capacity of 6
/// let addr_bounded = hannibal::build(MyActor)
///     .bounded(6)
///     .spawn();
///
/// //  start MyActor with an infinite mailbox
/// let addr_unbounded = hannibal::build(MyActor)
///     .unbounded()
///     .spawn();
/// ```
/// ## 2. Should the actor create a fresh object on restart?
/// If you restart the actor its [`started()`](`Actor::started`) method will be called.
/// You don't need to clean up and reset the actor's state if you configure it to be recreated from `Default` at spawn-time.
///
/// ### Example: Reset on Restart
/// This configuration will start the actor `Counter` with a bounded channel capacity of `6`
/// and recreate it from `Default` when `.restart()` is called.
/// ```no_run
/// # use hannibal_derive::{Actor, RestartableActor};
/// #[derive(Actor, RestartableActor, Default)]
/// struct Counter(usize);
///
/// let addr = hannibal::build(Counter(0))
///     .bounded(6)
///     .recreate_from_default()
///     .spawn();
/// ```
///
/// ## 3. Should it listen to a stream.
///
/// This one is an improvement over 0.10 where under the hood we would start listening to the stream on a separate task and send each message to the actor.
/// This ment two tasks and twice the amount of sending.
/// Since 0.12 hannibal can start the actor's event loop tightly coupled to the stream.
///
/// ### Example: Attache to stream
/// Here we tie the lifetime of the actor to the stream.
/// There is no extra task or messaging between them,
/// the actor owns the stream now and polls it in its own event loop
///
/// ```no_run
/// # use hannibal::{Context, StreamHandler, Handler, Actor};
/// # use hannibal_derive::{Actor};
/// #[derive(Actor)]
/// struct Connector;
///
/// impl StreamHandler<i32> for Connector {
///     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
///         println!("[Connection] Received: {}", msg);
///     }
/// }
///
/// let the_stream = futures::stream::iter(0..19);
///
/// let addr = hannibal::build(Connector)
///         .unbounded()
///         .non_restartable()
///         .with_stream(the_stream)
///         .spawn();
///
/// # let the_stream = futures::stream::iter(0..19);
///
/// // you can also express this shorter
/// let addr = hannibal::build(Connector)
///         .on_stream(the_stream)
///         .spawn();
///
/// # let the_stream = futures::stream::iter(0..19);
///
/// // you can also have bounded channels
/// let addr = hannibal::build(Connector)
///         .bounded_on_stream(10, the_stream)
///         .spawn();
/// ```
/// ## 4. Timeouts
///
/// One problem that you have to address when handling messages is that you can't wait forever.
/// You could handle this manualy inside every handler that you implement, but why bother?
/// Find more in the example `timeout.rs`.
///
/// ### Example: configure the actor to abort handling messages if they take too long
/// ```no_run
/// # use std::time::Duration;
/// # #[derive(Debug, Default, hannibal_derive::Actor)]
/// # struct SleepyActor(u8);
/// let mut addr = hannibal::build(SleepyActor(1))
///         .bounded(1)
///         .timeout(Duration::from_millis(100))
///         .spawn();
/// ```
///
/// ### Example: configure the actor to terminate if a message takes too long to handle
/// ```no_run
/// # use std::time::Duration;
/// # #[derive(Debug, Default, hannibal_derive::Actor)]
/// # struct SleepyActor(u8);
/// let mut addr = hannibal::build(SleepyActor(1))
///         .bounded(1)
///         .timeout(Duration::from_millis(100))
///         .fail_on_timeout(true)
///         .spawn_owning();
/// ```
/// In this particular example we want to hold an [`OwningAddr`](`crate::OwningAddr`) to the actor to receive the result of the actor's event loop.
///
/// ## 5. Register as a service
///
/// If the actor implements the [`Service`](`crate::Service`) trait,
/// you can also register it instead of spawning.
/// In this case, the actor's address can be accessed globally via [`Service::from_registry()`](`crate::Service::from_registry`).
///
/// ### Example: register service
/// ```no_run
/// # use hannibal_derive::{Actor, RestartableActor};
/// #[derive(Actor, RestartableActor, Default)]
/// struct Counter(usize);
/// impl hannibal::Service for Counter {}
///
/// # async move {
/// let addr = hannibal::build(Counter(0))
///     .bounded(6)
///     .recreate_from_default()
///     .register()
///     .await
///     .unwrap();
/// # };
/// ```
/// Instead of spawning the actor, which will return you the actor's address you can also register it as a service.
pub fn build<A: Actor>(actor: A) -> builder::BaseActorBuilder<A, spawner::DefaultSpawner> {
    builder::BaseActorBuilder::new(actor)
}
