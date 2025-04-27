use std::{marker::PhantomData, time::Duration};

use crate::{
    Addr, StreamHandler,
    actor::service::Service,
    addr::OwningAddr,
    channel::Channel,
    environment::{self, EnvironmentConfig},
};

use super::{
    Actor, RestartableActor,
    handle::ActorHandle,
    restart_strategy::{NonRestartable, RecreateFromDefault, RestartOnly, RestartStrategy},
};

#[derive(Default)]
pub struct BaseActorBuilder<A>
where
    A: Actor,
    // P: Spawner<A>,
{
    actor: A,
    // spawner: PhantomData<P>,
    config: EnvironmentConfig,
}

pub struct ActorBuilderWithChannel<A: Actor, R: RestartStrategy<A>>
where
    A: Actor,
{
    base: BaseActorBuilder<A>,
    channel: Channel<A>,
    restart: PhantomData<R>,
}

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
    // P: Spawner<A>,
{
    pub(crate) fn new(actor: A) -> Self {
        Self {
            actor,
            // spawner: PhantomData,
            config: Default::default(),
        }
    }

    const fn with_channel(self, channel: Channel<A>) -> ActorBuilderWithChannel<A, RestartOnly> {
        ActorBuilderWithChannel {
            base: self,
            restart: PhantomData,
            channel,
        }
    }

    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = Some(timeout);
        self
    }
    pub const fn fail_on_timeout(mut self, fail: bool) -> Self {
        self.config.fail_on_timeout = fail;
        self
    }

    pub fn bounded(self, capacity: usize) -> ActorBuilderWithChannel<A, RestartOnly> {
        self.with_channel(Channel::bounded(capacity))
    }

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
    // P: Spawner<A>,
{
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
    // P: Spawner<A>,
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
    // P: Spawner<A>,
    R: RestartStrategy<A> + 'static,
{
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
    pub fn spawn_owning(self) -> OwningAddr<A> {
        let ActorBuilderWithChannel {
            base: BaseActorBuilder { actor, config, .. },
            channel,
            ..
        } = self;

        let env = environment::Environment::<A, R>::from_channel(channel).with_config(config);
        let (event_loop, addr) = env.create_loop(actor);
        let handle = ActorHandle::spawn(event_loop);
        OwningAddr { addr, handle }
    }

    pub fn spawn(self) -> Addr<A> {
        let ActorBuilderWithChannel {
            base: BaseActorBuilder { actor, config, .. },
            channel,
            ..
        } = self;

        let env = environment::Environment::<A, R>::from_channel(channel).with_config(config);
        let (event_loop, addr) = env.create_loop(actor);
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

        let env = environment::Environment::<A, NonRestartable>::from_channel(channel)
            .with_config(config);
        let (event_loop, addr) = env.create_loop_on_stream(actor, stream);
        let _handle = ActorHandle::spawn(event_loop);
        addr
    }

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

        let env = environment::Environment::<A, NonRestartable>::from_channel(channel)
            .with_config(config);
        let (event_loop, addr) = env.create_loop_on_stream(actor, stream);
        let handle = ActorHandle::spawn(event_loop);
        OwningAddr { addr, handle }
    }
}
