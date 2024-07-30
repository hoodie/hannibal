use dyn_clone::DynClone;

use super::{tester::TestFn, ActorId, Message, Result};
use std::{
    future::Future,
    hash::{Hash, Hasher},
    pin::Pin,
};

pub(crate) trait CallerFn<T>: DynClone + Send + Sync + 'static
where
    T: Message,
{
    fn call(&self, msg: T) -> Pin<Box<dyn Future<Output = Result<T::Result>>>>;
}

impl<F, T> CallerFn<T> for F
where
    F: Fn(T) -> Pin<Box<dyn Future<Output = Result<T::Result>>>>,
    F: 'static + Send + Sync + DynClone,
    T: Message,
{
    fn call(&self, msg: T) -> Pin<Box<dyn Future<Output = Result<T::Result>>>> {
        self(msg)
    }
}

/// Caller of a specific message type
///
/// Like [`Sender<T>`](`super::Sender<T>`), `Caller` has a weak reference to the recipient of the message type,
/// and so will not prevent an actor from stopping if all [`Addr`](`crate::Addr`)'s have been dropped elsewhere.

pub struct Caller<T: Message> {
    /// Id of the corresponding [`Actor<A>`](crate::Actor<A>)
    pub actor_id: ActorId,
    pub(crate) caller_fn: Box<dyn CallerFn<T>>,
    pub(crate) test_fn: Box<dyn TestFn>,
}

impl<T: Message> Caller<T> {
    pub async fn call(&self, msg: T) -> Result<T::Result> {
        self.caller_fn.call(msg).await
    }
    pub fn can_upgrade(&self) -> bool {
        self.test_fn.test()
    }
}

impl<T: Message<Result = ()>> PartialEq for Caller<T> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id == other.actor_id
    }
}

impl<T: Message<Result = ()>> Hash for Caller<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.actor_id.hash(state);
    }
}

impl<T: Message> Clone for Caller<T> {
    fn clone(&self) -> Self {
        Self {
            actor_id: self.actor_id,
            caller_fn: dyn_clone::clone_box(&*self.caller_fn),
            test_fn: dyn_clone::clone_box(&*self.test_fn),
        }
    }
}
