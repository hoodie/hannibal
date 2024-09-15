use std::borrow::Cow;

use crate::{addr::StopReason, error::Result, lifecycle::LifeCycle, Addr, Context};

pub trait Message: 'static + Send {
    /// The return value type of the message
    /// This type can be set to () if the message does not return a value, or if it is a notification message
    type Result: 'static + Send;
    const TYPE_NAME: &'static str = "hannibal::Message";
}

/// Describes how to handle messages of a specific type.
/// Implementing Handler is a general way to handle incoming messages.
/// The type `M` is a message which can be handled by the actor.
pub trait Handler<M: Message>: Actor
where
    Self: std::marker::Sized,
{
    /// Method is called for every message received by this Actor.
    fn handle(
        &mut self,
        ctx: &mut Context<Self>,
        msg: M,
    ) -> impl futures::Future<Output = M::Result> + Send;
}

/// Describes how to handle messages of a specific type.
///
/// Implementing Handler is a general way to handle incoming streams.
/// The type `M` is a stream message which can be handled by the actor.
/// Stream messages do not need to implement the [`Message`] trait.
#[allow(unused_variables)]
pub trait StreamHandler<M: 'static>: Actor {
    /// Method is called for every message received by this Actor.
    fn handle(
        &mut self,
        ctx: &mut Context<Self>,
        msg: M,
    ) -> impl futures::Future<Output = ()> + Send;

    /// Method is called when stream get polled first time.
    fn started(&mut self, ctx: &mut Context<Self>) -> impl futures::Future<Output = ()> + Send {
        async {}
    }

    /// Method is called when stream finishes.
    ///
    /// By default this method stops actor execution.
    fn finished(&mut self, ctx: &mut Context<Self>) -> impl futures::Future<Output = ()> + Send {
        async { ctx.stop(None) }
    }

    /// Start an actor with a stream.
    ///
    /// This function starts an actor and adds this stream to the actor's context.
    /// This is a shorthand for calling [`Actor::start`] and then [`Context::add_stream`] in the actor's [`Actor::started`] method.
    ///
    /// But since you don't have access to the actor's context before the actor is started,
    /// you will need to store the stream in the actor like this:
    ///
    /// ```rust
    /// #[derive(Debug)]
    /// struct StartsStreamOnStarted(Option<futures::stream::Iter<Range<u32>>>);
    /// impl Actor for StartsStreamOnStarted {
    ///     async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()> {
    ///         println!("{self:?} started");
    ///         ctx.add_stream(self.0.take().unwrap());
    ///         Ok(())
    ///     }
    ///     async fn stopped(&mut self, _ctx: &mut Context<Self>) {
    ///         println!("{self:?} stopped");
    ///     }
    /// }
    ///
    /// StartsStreamOnStarted::from(0..10)
    ///     .start()
    ///     .await?
    ///     .wait_for_stop()
    ///     .await;
    /// ```
    ///
    /// This method lets you avoid this boilerplate.
    /// ```rust
    /// #[derive(Debug)]
    /// struct StopsOnFinished;
    /// impl Actor for StopsOnFinished {
    ///     async fn started(&mut self, _ctx: &mut Context<Self>) -> Result<()> {
    ///         println!("{self:?} started");
    ///         Ok(())
    ///     }
    ///     async fn stopped(&mut self, _ctx: &mut Context<Self>) {
    ///         println!("{self:?} stopped");
    ///     }
    /// }
    ///
    /// impl StreamHandler<u32> for StopsOnFinished {
    /// }
    ///
    /// // get stream added by lifecycle
    /// StopsOnFinished
    ///     .start_with_stream(stream::iter(0..10))
    ///     .await?
    ///     .wait_for_stop()
    ///     .await;
    ///
    /// ```

    fn start_with_stream<S>(
        self,
        stream: S,
    ) -> impl std::future::Future<Output = Result<Addr<Self>>> + Send
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        Self: crate::StreamHandler<S::Item>,
    {
        async { LifeCycle::new().start_with_stream(self, stream).await }
    }

    fn start_bound_to_stream<S>(
        self,
        stream: S,
    ) -> impl std::future::Future<Output = Result<Addr<Self>>> + Send
    where
        S: futures::Stream + Unpin + Send + 'static,
        S::Item: 'static + Send,
        Self: crate::StreamHandler<S::Item>,
    {
        async { LifeCycle::new().bind_to_stream(self, stream).await }
    }
}

/// Actors are objects which encapsulate state and behavior.
///
/// Actors run within a specific execution context [`Context<A>`].
/// The context object is available only during execution.
/// Each actor has a separate execution context.
///
/// Roles communicate by exchanging messages.
/// The requester can wait for a response.
/// By [`Addr`] referring to the actors, the actors must provide an [`Handler<M>`] implementation for this message.
/// All messages are statically typed.
#[allow(unused_variables)]
pub trait Actor: Sized + Send + 'static {
    const NAME: &'static str = "hannibal::Actor";

    /// Called when the actor is first started.
    fn started(
        &mut self,
        ctx: &mut Context<Self>,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called with stop reason before an actor is stopped.
    fn stopping(
        &mut self,
        ctx: &mut Context<Self>,
        reason: StopReason,
    ) -> impl std::future::Future<Output = ()> + Send {
        async {}
    }

    /// Called after an actor is stopped.
    fn stopped(&mut self, ctx: &mut Context<Self>) -> impl std::future::Future<Output = ()> + Send {
        async {}
    }

    /// Construct and start a new actor, returning its address.
    ///
    /// This is constructs a new actor using the [`Default`] trait, and invokes its [`start`](`Actor::start`) method.
    fn start_default() -> impl std::future::Future<Output = Result<Addr<Self>>> + Send
    where
        Self: Default,
    {
        async { Self::default().start().await }
    }

    /// Start a new actor, returning its address.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use hannibal::*;
    ///
    /// struct MyActor;
    ///
    /// impl Actor for MyActor {}
    ///
    /// #[message(result = i32)]
    /// struct MyMsg(i32);
    ///
    /// impl Handler<MyMsg> for MyActor {
    ///     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: MyMsg) -> i32 {
    ///         msg.0 * msg.0
    ///     }
    /// }
    ///
    /// #[hannibal::main]
    /// async fn main() -> Result<()> {
    ///     // Start actor and get its address
    ///     let mut addr = MyActor.start().await?;
    ///
    ///     // Send message `MyMsg` to actor via addr
    ///     let res = addr.call(MyMsg(10)).await?;
    ///     assert_eq!(res, 100);
    ///     Ok(())
    /// }
    /// ```
    fn start(self) -> impl std::future::Future<Output = Result<Addr<Self>>> + Send {
        async { LifeCycle::new().start_actor(self).await }
    }

    /// Start a new actor with a bounded buffer, returning its address.
    /// The buffer is the maximum number of messages that can be queued for the actor.
    /// If the buffer is full, the actor will stop processing messages until the buffer is no longer full.
    fn start_bounded(
        self,
        buffer: usize,
    ) -> impl std::future::Future<Output = Result<Addr<Self>>> + Send {
        async move { LifeCycle::new_bounded(buffer).start_actor(self).await }
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::from(Self::NAME)
    }
}
