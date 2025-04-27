use std::{any::type_name, pin::Pin, sync::Arc};

use crate::{Actor, DynResult};

/// A future that resolves to an actor.
pub type JoinFuture<A> = Pin<Box<dyn Future<Output = Option<A>> + Send>>;

pub struct ActorHandle<A> {
    join_fn: Box<dyn FnMut() -> JoinFuture<A>>,
    detach_fn: Option<Box<dyn FnOnce()>>,
}

impl<A> ActorHandle<A> {
    fn new<F>(join_fn: F) -> Self
    where
        F: FnMut() -> JoinFuture<A> + 'static,
    {
        Self {
            join_fn: Box::new(join_fn),
            detach_fn: None,
        }
    }

    #[cfg_attr(feature = "tokio_runtime", allow(dead_code))]
    fn with_detach_fn<F: FnOnce() + 'static>(mut self, detach_fn: F) -> Self {
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

impl<A: Actor> ActorHandle<A> {
    #[cfg(feature = "tokio_runtime")]
    pub fn spawn<F>(event_loop: F) -> Self
    where
        F: Future<Output = DynResult<A>> + Send + 'static,
    {
        log::trace!("spawning in tokio {}", type_name::<Self>());

        let join_handle = tokio::spawn(event_loop);
        let handle = Arc::new(async_lock::Mutex::new(Some(join_handle)));

        Self::new(move || -> JoinFuture<A> {
            let handle = Arc::clone(&handle);
            Box::pin(async move {
                if let Some(handle) = handle.lock().await.take() {
                    handle.await.ok().and_then(Result::ok)
                } else {
                    None
                }
            })
        })
    }

    #[cfg(not(feature = "tokio_runtime"))]
    pub fn spawn<F>(event_loop: F) -> Self
    where
        F: Future<Output = DynResult<A>> + Send + 'static,
    {
        log::trace!("spawning {}", type_name::<Self>());

        let task = async_global_executor::spawn(event_loop);
        let handle = Arc::new(async_lock::Mutex::new(Some(task)));
        let detach_handle = Arc::clone(&handle);

        Self::new(move || {
            let handle = Arc::clone(&handle);
            Box::pin(async move {
                if let Some(handle) = handle.lock().await.take() {
                    handle.await.ok()
                } else {
                    None
                }
            })
        })
        .with_detach_fn(move || {
            let mut handle = detach_handle.lock_blocking().take();
            if let Some(handle) = handle.take() {
                handle.detach();
            }
        })
    }
}
