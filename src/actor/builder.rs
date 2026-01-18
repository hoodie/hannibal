//! # Configure the runtime behavior of actor instances.
//!
//! Configuring an actor does not just mean setting up the data inside the `struct` that implements the actor,
//! hannibal lets you configure your actor's runtime behaving before spawning.
//! This includes
//! 1. What kind of channels do you use under the hood and how large are the channel's buffers?
//! 2. Should the actor create a fresh object on restart?
//! 3. Should it listen to a stream.
//! 4. Should the actor enforce timeouts when waiting?
//! 5. Register as a service
//!
//! ## 1. What kind of channels do you use under the hood and how large are the channel's buffers?
//! Unbounded versus Bounded.
//!
//! ### Example
//! You can configure what kind of channel the actor uses and what its capacity should be.
//!
//! ```no_run
//! # #[derive(hannibal_derive::Actor,hannibal_derive::RestartableActor, Default)]
//! # struct MyActor;
//! // start MyActor with a channel capacity of 6
//! let addr_bounded = hannibal::setup_actor(MyActor)
//!     .bounded(6)
//!     .spawn();
//!
//! //  start MyActor with an unbounded channel
//! let addr_unbounded = hannibal::setup_actor(MyActor)
//!     .unbounded()
//!     .spawn();
//! ```
//!
//! ## 2. Should the actor create a fresh object on restart?
//! If you restart the actor its [`started()`](`Actor::started`) method will be called.
//! You don't need to clean up and reset the actor's state if you configure it to be recreated from `Default` at spawn-time.
//!
//! ### Example: Reset on Restart
//! This configuration will start the actor `Counter` with a bounded channel capacity of `6`
//! and recreate it from `Default` when `.restart()` is called.
//! ```no_run
//! # use hannibal_derive::{Actor, RestartableActor};
//! #[derive(Actor, RestartableActor, Default)]
//! struct Counter(usize);
//!
//! let addr = hannibal::setup_actor(Counter(0))
//!     .bounded(6)
//!     .recreate_from_default()
//!     .spawn();
//! ```
//!
//! ## 3. Should it listen to a stream.
//!
//! This one is an improvement over 0.10 where under the hood we would start listening to the stream on a separate task and send each message to the actor.
//! This meant two tasks and twice the amount of sending.
//! Since 0.12 hannibal can start the actor's event loop tightly coupled to the stream.
//!
//! ### Example: Attach to stream
//! Here we tie the lifetime of the actor to the stream.
//! There is no extra task or messaging between them,
//! the actor owns the stream now and polls it in its own event loop.
//!
//! ```no_run
//! # use hannibal::{Context, StreamHandler, Handler, Actor};
//! # use hannibal_derive::{Actor};
//! #[derive(Actor)]
//! struct Connector;
//!
//! impl StreamHandler<i32> for Connector {
//!     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: i32) {
//!         println!("[Connection] Received: {}", msg);
//!     }
//! }
//!
//! let the_stream = futures::stream::iter(0..19);
//!
//! let addr = hannibal::setup_actor(Connector)
//!         .bounded(10)
//!         .on_stream(the_stream)
//!         .spawn();
//! ```
//!
//! ## 4. Timeouts
//!
//! One problem that you have to address when handling messages is that you can't wait forever.
//! You could handle this manually inside every handler that you implement, but why bother?
//! Find more in the example `timeout.rs`.
//!
//! ### Example: configure the actor to abort handling messages if they take too long
//! ```no_run
//! # use std::time::Duration;
//! # #[derive(Debug, Default, hannibal_derive::Actor)]
//! # struct SleepyActor(u8);
//! let mut addr = hannibal::setup_actor(SleepyActor(1))
//!         .bounded(1)
//!         .timeout(Duration::from_millis(100))
//!         .spawn();
//! ```
//!
//! ### Example: configure the actor to terminate if a message takes too long to handle
//! ```no_run
//! # use std::time::Duration;
//! # #[derive(Debug, Default, hannibal_derive::Actor)]
//! # struct SleepyActor(u8);
//! let mut addr = hannibal::setup_actor(SleepyActor(1))
//!         .timeout(Duration::from_millis(100))
//!         .fail_on_timeout(true)
//!         .bounded(1)
//!         .spawn_owning();
//! ```
//! In this particular example we want to hold an [`OwningAddr`](`crate::OwningAddr`) to the actor to receive the result of the actor's event loop.
//!
//! ## 5. Register as a service
//!
//! If the actor implements the [`Service`](`crate::Service`) trait,
//! you can also register it instead of spawning.
//! In this case, the actor's address can be accessed globally via [`Service::from_registry()`](`crate::Service::from_registry`).
//!
//! ### Example: register service
//! ```no_run
//! # use hannibal_derive::{Actor, RestartableActor};
//! #[derive(Actor, RestartableActor, Default)]
//! struct Counter(usize);
//! impl hannibal::Service for Counter {}
//!
//! # async move {
//! let addr = hannibal::setup_actor(Counter(0))
//!     .bounded(6)
//!     .recreate_from_default()
//!     .register()
//!     .await
//!     .unwrap();
//! # };
//! ```
//! Instead of spawning the actor, which will return you the actor's address you can also register it as a service.

use std::{marker::PhantomData, time::Duration};

use crate::{
    Addr, StreamHandler,
    actor::service::Service,
    addr::OwningAddr,
    channel::Channel,
    event_loop::{EventLoop, EventLoopConfig},
};

use super::{
    Actor, RestartableActor,
    handle::ActorHandle,
    restart_strategy::{NonRestartable, RecreateFromDefault, RestartOnly, RestartStrategy},
};

/// Start constructing an actor.
///
/// Find out more in the [module level documentation](`crate::builder`).
pub fn setup_actor<A: Actor>(actor: A) -> ActorBuilder<A> {
    ActorBuilder::new(actor)
}

/// Trait for building actors.
pub trait Configurable<A: Actor> {
    /// Build the actor.
    fn setup_actor(self) -> ActorBuilder<A>;
}

impl<A: Actor> Configurable<A> for A {
    fn setup_actor(self) -> ActorBuilder<A> {
        setup_actor(self)
    }
}

/// Builder for configuring and spawning an actor.
pub struct ActorBuilder<A: Actor, R: RestartStrategy<A> = RestartOnly> {
    actor: A,
    config: EventLoopConfig,
    channel: Option<Channel<A>>,
    _restart: PhantomData<R>,
}

impl<A: Actor> ActorBuilder<A, RestartOnly> {
    fn new(actor: A) -> Self {
        Self {
            actor,
            config: EventLoopConfig::default(),
            channel: None,
            _restart: PhantomData,
        }
    }
}

// Configuration methods available for all restart strategies
impl<A: Actor, R: RestartStrategy<A> + 'static> ActorBuilder<A, R> {
    /// Use a bounded channel for message passing.
    ///
    /// A bounded channel provides backpressure when the channel is full.
    #[must_use]
    pub fn bounded(mut self, capacity: usize) -> Self {
        self.channel = Some(Channel::bounded(capacity));
        self
    }

    /// Use an unbounded channel for message passing.
    ///
    /// This is the default. An unbounded channel will never block senders.
    #[must_use]
    pub fn unbounded(mut self) -> Self {
        self.channel = Some(Channel::unbounded());
        self
    }

    /// Define a timeout for handling messages.
    ///
    /// If a message handler takes longer than this duration, it will be aborted
    /// (or the actor will fail, if `fail_on_timeout` is set).
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = Some(timeout);
        self
    }

    /// Define whether the actor should fail on timeout.
    ///
    /// If `true`, the actor will terminate when a timeout is exceeded.
    /// If `false` (default), the handler is aborted but the actor continues.
    #[must_use]
    pub const fn fail_on_timeout(mut self, fail: bool) -> Self {
        self.config.fail_on_timeout = fail;
        self
    }

    fn resolve_channel(self) -> (A, EventLoopConfig, Channel<A>) {
        let channel = self.channel.unwrap_or_else(Channel::unbounded);
        (self.actor, self.config, channel)
    }

    /// Spawn the actor and return an address.
    pub fn spawn(self) -> Addr<A> {
        self.spawn_owning().detach()
    }

    /// Spawn the actor and return an owning address.
    ///
    /// An owning address allows retrieving the actor after it stops.
    pub fn spawn_owning(self) -> OwningAddr<A> {
        let (actor, config, channel) = self.resolve_channel();

        let (event_loop, addr) = EventLoop::<A, R>::from_channel(channel)
            .with_config(config)
            .create(actor);
        let handle = ActorHandle::spawn(event_loop);
        OwningAddr { addr, handle }
    }
}

// Stream attachment - only available for RestartOnly (default) strategy
impl<A: Actor> ActorBuilder<A, RestartOnly> {
    /// Attach a stream to this actor.
    ///
    /// The actor will process messages from the stream alongside regular messages.
    /// Stream-attached actors cannot be restarted.
    pub fn on_stream<S>(self, stream: S) -> StreamActorBuilder<A, S>
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: StreamHandler<S::Item>,
    {
        StreamActorBuilder {
            actor: self.actor,
            config: self.config,
            channel: self.channel,
            stream,
        }
    }
}

// Restart strategy transitions
impl<A, R> ActorBuilder<A, R>
where
    A: RestartableActor + Default,
    R: RestartStrategy<A> + 'static,
{
    /// Configure the actor to construct a new instance from `Default::default()` when restarting.
    ///
    /// This is useful when you want a clean slate on restart rather than just
    /// calling the lifecycle hooks on the existing instance.
    #[must_use]
    pub fn recreate_from_default(self) -> ActorBuilder<A, RecreateFromDefault> {
        ActorBuilder {
            actor: self.actor,
            config: self.config,
            channel: self.channel,
            _restart: PhantomData,
        }
    }
}

// Service registration
impl<A, R> ActorBuilder<A, R>
where
    A: Actor + Service,
    R: RestartStrategy<A> + 'static,
{
    /// Register an actor as a service.
    ///
    /// The actor can then be accessed globally via [`Service::from_registry()`].
    pub async fn register(self) -> crate::error::Result<(Addr<A>, Option<Addr<A>>)> {
        self.spawn().register().await
    }
}

/// Builder for an actor attached to a stream.
///
/// Stream-attached actors cannot be restarted.
pub struct StreamActorBuilder<A, S>
where
    A: Actor + StreamHandler<S::Item>,
    S: futures::Stream + Unpin + Send + 'static,
    S::Item: 'static + Send,
{
    actor: A,
    config: EventLoopConfig,
    channel: Option<Channel<A>>,
    stream: S,
}

impl<A, S> StreamActorBuilder<A, S>
where
    A: Actor + StreamHandler<S::Item>,
    S: futures::Stream + Unpin + Send + 'static,
    S::Item: 'static + Send,
{
    /// Use a bounded channel for message passing.
    ///
    /// A bounded channel provides backpressure when the channel is full.
    #[must_use]
    pub fn bounded(mut self, capacity: usize) -> Self {
        self.channel = Some(Channel::bounded(capacity));
        self
    }

    /// Use an unbounded channel for message passing.
    ///
    /// This is the default. An unbounded channel will never block senders.
    #[must_use]
    pub fn unbounded(mut self) -> Self {
        self.channel = Some(Channel::unbounded());
        self
    }

    /// Define a timeout for handling messages.
    ///
    /// If a message handler takes longer than this duration, it will be aborted
    /// (or the actor will fail, if `fail_on_timeout` is set).
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = Some(timeout);
        self
    }

    /// Define whether the actor should fail on timeout.
    ///
    /// If `true`, the actor will terminate when a timeout is exceeded.
    /// If `false` (default), the handler is aborted but the actor continues.
    #[must_use]
    pub const fn fail_on_timeout(mut self, fail: bool) -> Self {
        self.config.fail_on_timeout = fail;
        self
    }

    fn resolve_channel(self) -> (A, EventLoopConfig, Channel<A>, S) {
        let channel = self.channel.unwrap_or_else(Channel::unbounded);
        (self.actor, self.config, channel, self.stream)
    }

    /// Spawn the actor and return an address.
    pub fn spawn(self) -> Addr<A> {
        self.spawn_owning().detach()
    }

    /// Spawn the actor and return an owning address.
    ///
    /// An owning address allows retrieving the actor after it stops.
    pub fn spawn_owning(self) -> OwningAddr<A> {
        let (actor, config, channel, stream) = self.resolve_channel();

        let (event_loop, addr) = EventLoop::<A, NonRestartable>::from_channel(channel)
            .with_config(config)
            .create_on_stream(actor, stream);
        let handle = ActorHandle::spawn(event_loop);
        OwningAddr { addr, handle }
    }
}
