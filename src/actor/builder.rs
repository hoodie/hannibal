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
    restart_strategy::{NonRestartable, RecreateFromDefault, RestartOnly, RestartStrategy},
    spawner::Spawner,
};

#[derive(Default)]
pub struct BaseActorBuilder<A, P>
where
    A: Actor,
    P: Spawner<A>,
{
    actor: A,
    spawner: PhantomData<P>,
    config: EnvironmentConfig,
}

pub struct ActorBuilderWithChannel<A: Actor, P, R: RestartStrategy<A>>
where
    A: Actor,
    P: Spawner<A>,
{
    base: BaseActorBuilder<A, P>,
    channel: Channel<A>,
    restart: PhantomData<R>,
}

pub struct StreamActorBuilder<A, P, S>
where
    S: futures::Stream + Unpin + Send + 'static,
    S::Item: 'static + Send,
    A: StreamHandler<S::Item>,
    P: Spawner<A>,
{
    with_channel: ActorBuilderWithChannel<A, P, NonRestartable>,
    stream: S,
}

/// add channel
impl<A, P> BaseActorBuilder<A, P>
where
    A: Actor,
    P: Spawner<A>,
{
    pub(crate) fn new(actor: A) -> Self {
        Self {
            actor,
            spawner: PhantomData,
            config: Default::default(),
        }
    }

    const fn with_channel(self, channel: Channel<A>) -> ActorBuilderWithChannel<A, P, RestartOnly> {
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

    pub fn bounded(self, capacity: usize) -> ActorBuilderWithChannel<A, P, RestartOnly> {
        self.with_channel(Channel::bounded(capacity))
    }

    pub fn unbounded(self) -> ActorBuilderWithChannel<A, P, RestartOnly> {
        self.with_channel(Channel::unbounded())
    }

    /// Create a non-restarable on that stream
    pub fn bounded_on_stream<S>(self, capacity: usize, stream: S) -> StreamActorBuilder<A, P, S>
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        A: StreamHandler<S::Item>,
    {
        self.with_channel(Channel::bounded(capacity))
            .non_restartable()
            .with_stream(stream)
    }

    /// Create a non-restarable on that stream
    pub fn on_stream<S>(self, stream: S) -> StreamActorBuilder<A, P, S>
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
impl<A, P> ActorBuilderWithChannel<A, P, NonRestartable>
where
    A: Actor,
    P: Spawner<A>,
{
    pub fn with_stream<S>(self, stream: S) -> StreamActorBuilder<A, P, S>
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
impl<A, P, R> ActorBuilderWithChannel<A, P, R>
where
    A: Actor,
    P: Spawner<A>,
    R: RestartStrategy<A> + 'static,
{
    pub fn non_restartable(self) -> ActorBuilderWithChannel<A, P, NonRestartable> {
        ActorBuilderWithChannel {
            base: self.base,
            channel: self.channel,
            restart: PhantomData,
        }
    }
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.base.config.timeout = Some(timeout);
        self
    }
    pub const fn fail_on_timeout(mut self, fail: bool) -> Self {
        self.base.config.fail_on_timeout = fail;
        self
    }
}

/// make recreate from `Default` on restart
impl<A, P, R> ActorBuilderWithChannel<A, P, R>
where
    A: RestartableActor + Default,
    P: Spawner<A>,
    R: RestartStrategy<A> + 'static,
{
    pub fn recreate_from_default(self) -> ActorBuilderWithChannel<A, P, RecreateFromDefault> {
        ActorBuilderWithChannel {
            base: self.base,
            channel: self.channel,
            restart: PhantomData,
        }
    }
}

/// spawn actor
impl<A, P, R> ActorBuilderWithChannel<A, P, R>
where
    A: Actor,
    P: Spawner<A>,
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
        let handle = P::spawn_actor(event_loop);
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
        let _handle = P::spawn_actor(event_loop);
        addr
    }
}

/// register service
impl<A, P, R> ActorBuilderWithChannel<A, P, R>
where
    A: Actor + Service,
    P: Spawner<A>,
    R: RestartStrategy<A> + 'static,
{
    pub async fn register(self) -> crate::error::Result<(Addr<A>, Option<Addr<A>>)> {
        self.spawn().register().await
    }
}

/// spawn actor on stream, non restartable
impl<A, P, S> StreamActorBuilder<A, P, S>
where
    S: futures::Stream + Unpin + Send + 'static,
    S::Item: 'static + Send,
    A: StreamHandler<S::Item>,
    A: Actor,
    P: Spawner<A>,
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
        let _handle = P::spawn_actor(event_loop);
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
        let handle = P::spawn_actor(event_loop);
        OwningAddr { addr, handle }
    }
}
