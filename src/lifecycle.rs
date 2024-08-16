use super::*;
pub struct LifeCycle {
    ctx: Context,
}

impl Default for LifeCycle {
    fn default() -> Self {
        Self::new()
    }
}

impl LifeCycle {
    pub fn new() -> Self {
        LifeCycle {
            ctx: Context::new(),
        }
    }

    pub fn start<A>(mut self, actor: Arc<A>) -> Addr<A>
    where
        A: Actor + Send + Sync + 'static,
    {
        let LifeCycle { ref mut ctx } = self;
        ctx.spawn(actor.clone());
        Addr {
            ctx: Arc::new(Mutex::new(self.ctx)),
            actor,
        }
    }
}
