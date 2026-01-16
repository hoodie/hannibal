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
//! let addr_bounded = hannibal::build(MyActor)
//!     .bounded(6)
//!     .spawn();
//!
//! //  start MyActor with an unbounded channel
//! let addr_unbounded = hannibal::build(MyActor)
//!     .unbounded()
//!     .spawn();
//! ```
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
//! let addr = hannibal::build(Counter(0))
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
//! ### Example: Attache to stream
//! Here we tie the lifetime of the actor to the stream.
//! There is no extra task or messaging between them,
//! the actor owns the stream now and polls it in its own event loop
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
//! let addr = hannibal::build(Connector)
//!         .unbounded()
//!         .non_restartable()
//!         .with_stream(the_stream)
//!         .spawn();
//!
//! # let the_stream = futures::stream::iter(0..19);
//!
//! // you can also express this shorter
//! let addr = hannibal::build(Connector)
//!         .on_stream(the_stream)
//!         .spawn();
//!
//! # let the_stream = futures::stream::iter(0..19);
//!
//! // you can also have bounded channels
//! let addr = hannibal::build(Connector)
//!         .bounded_on_stream(10, the_stream)
//!         .spawn();
//! ```
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
//! let mut addr = hannibal::build(SleepyActor(1))
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
//! let mut addr = hannibal::build(SleepyActor(1))
//!         .bounded(1)
//!         .timeout(Duration::from_millis(100))
//!         .fail_on_timeout(true)
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
//! let addr = hannibal::build(Counter(0))
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
pub fn build<A: Actor>(actor: A) -> BaseActorBuilder<A> {
    BaseActorBuilder::new(actor)
}

/// Base builder for launching an actor.
#[derive(Default)]
pub struct BaseActorBuilder<A>
where
    A: Actor,
{
    actor: A,
    config: EventLoopConfig,
}

/// Builder stage to configure the channel used in the runtime of this actor instance.
pub struct ActorBuilderWithChannel<A: Actor, R: RestartStrategy<A>>
where
    A: Actor,
{
    base: BaseActorBuilder<A>,
    channel: Channel<A>,
    restart: PhantomData<R>,
}

/// Builder for launching an actor connected to a stream.
pub struct StreamActorBuilder<A, S>
where
    S: futures::Stream + Unpin + Send + 'static,
    S::Item: 'static + Send,
    A: StreamHandler<S::Item>,
{
    with_channel: ActorBuilderWithChannel<A, NonRestartable>,
    stream: S,
}

/// add channel
impl<A> BaseActorBuilder<A>
where
    A: Actor,
{
    pub(crate) fn new(actor: A) -> Self {
        Self {
            actor,
            config: Default::default(),
        }
    }

    /// Define the kind of channel
    const fn with_channel(self, channel: Channel<A>) -> ActorBuilderWithChannel<A, RestartOnly> {
        ActorBuilderWithChannel {
            base: self,
            restart: PhantomData,
            channel,
        }
    }

    /// Define a timeout for handling messages.
    ///
    /// To avoid blocking the actor, a timeout can be set.
    /// If the timeout is reached, the message handling will either be aborted or the entire actor will be terminated.
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = Some(timeout);
        self
    }

    /// Define whether the actor should fail on timeout.
    ///
    /// If the timeout is reached the entire actor will be terminated.
    pub const fn fail_on_timeout(mut self, fail: bool) -> Self {
        self.config.fail_on_timeout = fail;
        self
    }

    /// Use a bounded channel for message passing.
    pub fn bounded(self, capacity: usize) -> ActorBuilderWithChannel<A, RestartOnly> {
        self.with_channel(Channel::bounded(capacity))
    }

    /// Use an unbounded channel for message passing.
    pub fn unbounded(self) -> ActorBuilderWithChannel<A, RestartOnly> {
        self.with_channel(Channel::unbounded())
    }

    /// Create a non-restartable on that stream
    pub fn bounded_on_stream<S>(self, capacity: usize, stream: S) -> StreamActorBuilder<A, S>
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: StreamHandler<S::Item>,
    {
        self.with_channel(Channel::bounded(capacity))
            .non_restartable()
            .with_stream(stream)
    }

    /// Create a non-restartable on that stream
    pub fn on_stream<S>(self, stream: S) -> StreamActorBuilder<A, S>
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: StreamHandler<S::Item>,
    {
        self.with_channel(Channel::unbounded())
            .non_restartable()
            .with_stream(stream)
    }
}

/// add stream
impl<A> ActorBuilderWithChannel<A, NonRestartable>
where
    A: Actor,
{
    /// Construct an actor using an existing stream.
    pub fn with_stream<S>(self, stream: S) -> StreamActorBuilder<A, S>
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: StreamHandler<S::Item>,
    {
        StreamActorBuilder {
            with_channel: self.non_restartable(),
            stream,
        }
    }
}

/// make non restartable
impl<A, R> ActorBuilderWithChannel<A, R>
where
    A: Actor,
    R: RestartStrategy<A> + 'static,
{
    /// Build a non-restartable Actor.
    ///
    /// Only non-restartable actors can handle streams.
    pub fn non_restartable(self) -> ActorBuilderWithChannel<A, NonRestartable> {
        ActorBuilderWithChannel {
            base: self.base,
            channel: self.channel,
            restart: PhantomData,
        }
    }

    /// Set a maximum time that a handler can take to
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.base.config.timeout = Some(timeout);
        self
    }

    /// Terminate the actor if a timeout is exceeded.
    /// TODO: add a `.restart_on_timeout()` method
    pub const fn fail_on_timeout(mut self, fail: bool) -> Self {
        self.base.config.fail_on_timeout = fail;
        self
    }
}

/// make recreate from `Default` on restart
impl<A, R> ActorBuilderWithChannel<A, R>
where
    A: RestartableActor + Default,
    R: RestartStrategy<A> + 'static,
{
    /// Configure the actor to construct a new instance from `Default::default()` when restarting.
    pub fn recreate_from_default(self) -> ActorBuilderWithChannel<A, RecreateFromDefault> {
        ActorBuilderWithChannel {
            base: self.base,
            channel: self.channel,
            restart: PhantomData,
        }
    }
}

/// spawn actor
impl<A, R> ActorBuilderWithChannel<A, R>
where
    A: Actor,
    R: RestartStrategy<A> + 'static,
{
    /// Spawn the actor and return an owning address.
    pub fn spawn_owning(self) -> OwningAddr<A> {
        let ActorBuilderWithChannel {
            base: BaseActorBuilder { actor, config, .. },
            channel,
            ..
        } = self;

        let (event_loop, addr) = EventLoop::<A, R>::from_channel(channel)
            .with_config(config)
            .create(actor);
        let handle = ActorHandle::spawn(event_loop);
        OwningAddr { addr, handle }
    }

    /// Spawn the actor.
    pub fn spawn(self) -> Addr<A> {
        let ActorBuilderWithChannel {
            base: BaseActorBuilder { actor, config, .. },
            channel,
            ..
        } = self;

        let (event_loop, addr) = EventLoop::<A, R>::from_channel(channel)
            .with_config(config)
            .create(actor);
        ActorHandle::spawn(event_loop).detach();
        addr
    }
}

/// register service
impl<A, R> ActorBuilderWithChannel<A, R>
where
    A: Actor + Service,
    R: RestartStrategy<A> + 'static,
{
    /// Register an actor as a service.
    ///
    /// see [`crate::Addr::register`]
    pub async fn register(self) -> crate::error::Result<(Addr<A>, Option<Addr<A>>)> {
        self.spawn().register().await
    }
}

/// spawn actor on stream, non restartable
impl<A, S> StreamActorBuilder<A, S>
where
    S: futures::Stream + Unpin + Send + 'static,
    S::Item: 'static + Send,
    A: StreamHandler<S::Item>,
    A: Actor,
{
    /// Spawn the actor.
    pub fn spawn(self) -> Addr<A> {
        let Self {
            with_channel:
                ActorBuilderWithChannel {
                    base: BaseActorBuilder { actor, config, .. },
                    channel,
                    ..
                },
            stream,
        } = self;

        let (event_loop, addr) = EventLoop::<A, NonRestartable>::from_channel(channel)
            .with_config(config)
            .create_on_stream(actor, stream);
        ActorHandle::spawn(event_loop).detach();
        addr
    }

    /// Spawn the actor and return an owning address.
    pub fn spawn_owning(self) -> OwningAddr<A> {
        let Self {
            with_channel:
                ActorBuilderWithChannel {
                    base: BaseActorBuilder { actor, config, .. },
                    channel,
                    ..
                },
            stream,
        } = self;

        let (event_loop, addr) = EventLoop::<A, NonRestartable>::from_channel(channel)
            .with_config(config)
            .create_on_stream(actor, stream);
        let handle = ActorHandle::spawn(event_loop);
        OwningAddr { addr, handle }
    }
}
