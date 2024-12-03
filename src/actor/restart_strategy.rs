use std::future::Future;

use crate::{context::Context, Actor, DynResult};

pub trait RestartStrategy<A> {
    fn refresh(actor: A, ctx: &mut Context<A>) -> impl Future<Output = DynResult<A>> + Send;
}

#[derive(Clone, Copy, Debug)]
pub struct NonRestartable;
impl<A: Actor> RestartStrategy<A> for NonRestartable {
    async fn refresh(actor: A, _: &mut Context<A>) -> DynResult<A> {
        Ok(actor)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RestartOnly;
impl<A: Actor> RestartStrategy<A> for RestartOnly {
    async fn refresh(mut actor: A, ctx: &mut Context<A>) -> DynResult<A> {
        actor.stopped(ctx).await;
        actor.started(ctx).await?;
        Ok(actor)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RecreateFromDefault;
impl<A: Actor + Default> RestartStrategy<A> for RecreateFromDefault {
    async fn refresh(mut actor: A, ctx: &mut Context<A>) -> DynResult<A> {
        eprintln!("recreating refresh");
        actor.stopped(ctx).await;
        actor = A::default();
        actor.started(ctx).await?;
        Ok(actor)
    }
}

pub trait RestartableActor: Actor {}
