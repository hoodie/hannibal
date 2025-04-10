use std::pin::Pin;

/// A future that resolves to an actor.
pub type JoinFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

pub struct ActorHandle<A> {
    join_fn: Box<dyn FnMut() -> JoinFuture<A>>,
    detach_fn: Option<Box<dyn FnOnce()>>,
}

impl<A> ActorHandle<A> {
    pub fn new<F>(join_fn: F) -> Self
    where
        F: FnMut() -> JoinFuture<A> + 'static,
    {
        Self {
            join_fn: Box::new(join_fn),
            detach_fn: None,
        }
    }

    pub fn with_detach_fn<F: FnOnce() + 'static>(mut self, detach_fn: F) -> Self {
        self.detach_fn = Some(Box::new(detach_fn));
        self
    }

    pub fn join(&mut self) -> JoinFuture<A> {
        (self.join_fn)()
    }

    pub fn detach(self) {
        if let Some(detach_fn) = self.detach_fn {
            detach_fn();
        }
    }
}
