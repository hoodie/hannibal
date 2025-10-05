use std::sync::{Arc, RwLock, Weak};

use crate::{ActorResult, Context, Sender, actor::Actor, error::ActorError, handler::Handler};

pub struct Addr<A: Actor> {
    pub(crate) ctx: Arc<Context<Self>>,
    pub(crate) actor: Arc<RwLock<A>>,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Addr {
            ctx: self.ctx.clone(),
            actor: self.actor.clone(),
        }
    }
}

impl<A: Actor> Addr<A> {
    pub fn send<M>(&self, msg: M) -> ActorResult<()>
    where
        A: Handler<M> + 'static,
        M: Send + Sync + 'static,
    {
        self.ctx.send(msg, self.actor.clone())?;
        Ok(())
    }

    pub fn stop(&self) -> ActorResult<()> {
        self.ctx.stop()
    }

    pub fn downgrade(&self) -> WeakAddr<A> {
        WeakAddr {
            ctx: Arc::downgrade(&self.ctx),
            actor: Arc::downgrade(&self.actor),
        }
    }

    pub fn sender<M>(&self) -> Sender<M>
    where
        A: Handler<M> + 'static,
        M: Send + 'static,
    {
        (*self).clone().into()
    }
}

#[derive(Clone)]
pub struct WeakAddr<A: Actor> {
    ctx: Weak<Context<A>>,
    actor: Weak<RwLock<A>>,
}

impl<A: Actor> WeakAddr<A> {
    pub fn upgrade(&self) -> Option<Addr<A>> {
        Some(Addr {
            ctx: self.ctx.upgrade()?,
            actor: self.actor.upgrade()?,
        })
    }

    pub fn try_send<M>(&self, msg: M) -> ActorResult<()>
    where
        A: Handler<M> + 'static,
        M: Send + Sync + 'static,
    {
        if let Some(addr) = self.upgrade() {
            addr.send(msg)?;
            Ok(())
        } else {
            Err(ActorError::AlreadyStopped)
        }
    }
}
