use anyhow::anyhow;
use dyn_clone::DynClone;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
};

use super::{Message, Result};

pub(crate) trait CallerFn<T>: Send + Sync + 'static + DynClone
where
    T: Message,
{
    fn call(&self, msg: T) -> Pin<Box<dyn Future<Output = Result<T::Result>>>>;
}

impl<F, T> CallerFn<T> for F
where
    F: Fn(T) -> Pin<Box<dyn Future<Output = Result<T::Result>>>>,
    F: 'static + Send + Sync,
    F: DynClone,
    T: Message,
{
    fn call(&self, msg: T) -> Pin<Box<dyn Future<Output = Result<T::Result>>>> {
        self(msg)
    }
}

/// Caller of a specific message type.
pub struct Caller<T: Message> {
    pub(crate) call_fn: Arc<dyn CallerFn<T>>,
}

impl<T: Message> Clone for Caller<T> {
    fn clone(&self) -> Self {
        Caller {
            call_fn: self.call_fn.clone(),
        }
    }
}

/// Caller of a specific message type. You need to upgrade it to a `Caller` before you can use it.
///
/// Like [`WeakSender<T>`](`super::WeakSender<T>`), `Caller` has a weak reference to the recipient of the message type,
/// and so will not prevent an actor from stopping if all [`Addr`](`crate::Addr`)'s have been dropped elsewhere.
pub struct WeakCaller<T: Message> {
    pub(crate) call_fn: Weak<dyn CallerFn<T>>,
}

impl<T: Message> Clone for WeakCaller<T> {
    fn clone(&self) -> Self {
        WeakCaller {
            call_fn: self.call_fn.clone(),
        }
    }
}

impl<T: Message> Caller<T> {
    pub async fn call(&self, msg: T) -> Result<T::Result> {
        self.call_fn.call(msg).await
    }

    pub fn downgrade(&self) -> WeakCaller<T> {
        WeakCaller {
            call_fn: Arc::downgrade(&self.call_fn),
        }
    }
}

impl<T: Message> WeakCaller<T> {
    pub fn upgrade(&self) -> Option<Caller<T>> {
        self.call_fn.upgrade().map(|call_fn| Caller { call_fn })
    }

    pub fn can_upgrade(&self) -> bool {
        Weak::strong_count(&self.call_fn) > 1
    }

    pub async fn try_call(&self, msg: T) -> Result<T::Result> {
        let call_fn = self
            .call_fn
            .upgrade()
            .ok_or_else(|| anyhow!("Actor dropped"))?;
        call_fn.call(msg).await
    }
}
