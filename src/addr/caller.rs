use dyn_clone::DynClone;

use super::{tester::TestFn, ActorId, Message, Result};
use std::{
    future::Future,
    hash::{Hash, Hasher},
    pin::Pin,
};

pub(crate) trait CallerFn<M: Message>: DynClone + Send + Sync + 'static {
    fn call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>>;
}

impl<F, M> CallerFn<M> for F
where
    F: Fn(M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>>,
    F: 'static + Send + Sync + DynClone,
    M: Message,
{
    fn call(&self, msg: M) -> Pin<Box<dyn Future<Output = Result<M::Result>>>> {
        self(msg)
    }
}

/// Caller of a specific message type
///
/// Like [`Sender<M>`](`super::Sender<M>`), `Caller` has a weak reference to the recipient of the message type,
/// and so will not prevent an actor from stopping if all [`Addr`](`crate::Addr`)'s have been dropped elsewhere.

pub struct Caller<M: Message> {
    /// Id of the corresponding [`Actor<A>`](crate::Actor<A>)
    pub actor_id: ActorId,
    pub(crate) caller_fn: Box<dyn CallerFn<M>>,
    pub(crate) test_fn: Box<dyn TestFn>,
}

impl<M: Message> Caller<M> {
    pub async fn call(&self, msg: M) -> Result<M::Result> {
        self.caller_fn.call(msg).await
    }
    pub fn can_upgrade(&self) -> bool {
        self.test_fn.test()
    }
}

impl<M: Message<Result = ()>> PartialEq for Caller<M> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id == other.actor_id
    }
}

impl<M: Message<Result = ()>> Hash for Caller<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.actor_id.hash(state);
    }
}

impl<M: Message> Clone for Caller<M> {
    fn clone(&self) -> Self {
        Self {
            actor_id: self.actor_id,
            caller_fn: dyn_clone::clone_box(&*self.caller_fn),
            test_fn: dyn_clone::clone_box(&*self.test_fn),
        }
    }
}
