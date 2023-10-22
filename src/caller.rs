use dyn_clone::DynClone;

use crate::{ActorId, Message, Result};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;

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

pub(crate) trait SenderFn<T>: DynClone + 'static + Send + Sync
where
    T: Message,
{
    fn send(&self, msg: T) -> Result<()>;
}

impl<F, T> SenderFn<T> for F
where
    F: Fn(T) -> Result<()>,
    F: 'static + Send + Sync + Clone,
    T: Message,
{
    fn send(&self, msg: T) -> Result<()> {
        self(msg)
    }
}

pub(crate) trait TestFn: DynClone + 'static + Send + Sync {
    fn test(&self) -> bool;
}

impl<F> TestFn for F
where
    F: Fn() -> bool,
    F: DynClone + 'static + Send + Sync,
{
    fn test(&self) -> bool {
        self()
    }
}

/// Caller of a specific message type
///
/// Like [`Sender<T>`], `Caller` has a weak reference to the recipient of the message type,
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

/// Sender of a specific message type
///
/// Like [`Caller<T>`], Sender has a weak reference to the recipient of the message type,
/// and so will not prevent an actor from stopping if all [`Addr`](`crate::Addr`)'s have been dropped elsewhere.
/// This allows it to be used in the `send_later` and `send_interval` actor functions,
/// and not keep the actor alive indefinitely even after all references to it have been dropped (unless `ctx.stop()` is called from within)

pub struct Sender<T: Message> {
    /// Id of the corresponding [`Actor<A>`](crate::actor::Actor)
    pub actor_id: ActorId,
    pub(crate) sender_fn: Box<dyn SenderFn<T>>,
    pub(crate) test_fn: Box<dyn TestFn>,
}

impl<T: Message<Result = ()>> Sender<T> {
    pub fn send(&self, msg: T) -> Result<()> {
        self.sender_fn.send(msg)
    }
    pub fn can_upgrade(&self) -> bool {
        self.test_fn.test()
    }
}

impl<T: Message<Result = ()>> PartialEq for Sender<T> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id == other.actor_id
    }
}

impl<T: Message<Result = ()>> Hash for Sender<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.actor_id.hash(state);
    }
}
