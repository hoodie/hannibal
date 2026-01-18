use std::future::Future;

use crate::{Actor, DynResult, context::Context};

pub trait RestartStrategy<A: Actor> {
    fn refresh(actor: A, ctx: &mut Context<A>) -> impl Future<Output = DynResult<A>> + Send;
}

#[derive(Clone, Copy, Debug)]
pub struct NonRestartable;
impl<A: Actor> RestartStrategy<A> for NonRestartable {
    async fn refresh(actor: A, _: &mut Context<A>) -> DynResult<A> {
        Ok(actor)
    }
}

/// Use the the actors start and stop methods to reset.
///
/// If you call [`Context::restart()`] on an [`Actor`] that implements `RestartOnly` this will call the actor's  [`Actor::stopped()`] and [`Actor::started()`] methods in order.
#[derive(Clone, Copy, Debug)]
pub struct RestartOnly;
impl<A: Actor> RestartStrategy<A> for RestartOnly {
    async fn refresh(mut actor: A, ctx: &mut Context<A>) -> DynResult<A> {
        actor.stopped(ctx).await;
        actor.started(ctx).await?;
        Ok(actor)
    }
}

/// Recreate a new instance before restart.
///
/// If you call [`Context::restart()`] on an [`Actor`] that implements `RecreateFromDefault` this will call the actor's [`Actor::stopped()`] method,
/// create a new instance of the actor using [`Default::default()`], and then call the actor's [`Actor::started()`] method.
#[derive(Clone, Copy, Debug)]
pub struct RecreateFromDefault;
impl<A: Actor + Default> RestartStrategy<A> for RecreateFromDefault {
    async fn refresh(mut actor: A, ctx: &mut Context<A>) -> DynResult<A> {
        log::trace!("recreating refresh");
        actor.stopped(ctx).await;
        actor = A::default();
        actor.started(ctx).await?;
        Ok(actor)
    }
}

/// A marker trait for actors that can be restarted.
pub trait RestartableActor: Actor {}
