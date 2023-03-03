use dyn_clone::DynClone;

use crate::{ActorId, Message, Result};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;

pub trait CallerFn<T: Message> {
    fn call(&self, msg: T) -> Pin<Box<dyn Future<Output = Result<T::Result>>>>;
}

impl<F, T> CallerFn<T> for F
where
    F: Fn(T) -> Pin<Box<dyn Future<Output = Result<T::Result>>>> + 'static + Send + Sync + Clone,
    T: Message,
{
    fn call(&self, msg: T) -> Pin<Box<dyn Future<Output = Result<T::Result>>>> {
        (self)(msg)
    }
}

pub trait SenderFn<T>: DynClone + 'static + Send + Sync
where
    T: Message,
{
    fn send(&self, msg: T) -> Result<()>;
}

impl<F, T> SenderFn<T> for F
where
    F: Fn(T) -> Result<()> + 'static + Send + Sync + Clone,
    T: Message,
{
    fn send(&self, msg: T) -> Result<()> {
        self(msg)
    }
}

// pub(crate) type TestFn = Box<dyn Fn() -> bool + 'static + Send>;
pub trait TestFn: Send {
    fn test(&self) -> bool;
}

impl<F> TestFn for F
where
    F: Fn() -> bool + Send,
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
    // pub(crate) test_fn: Arc<Mutex<TestFn>>,
    pub(crate) test_fn: Box<dyn TestFn>,
}

impl<T: Message> Caller<T> {
    pub fn call(&self, msg: T) -> Pin<Box<dyn Future<Output = Result<T::Result>>>> {
        self.caller_fn.call(msg)
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
        self.actor_id.hash(state)
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
    // pub(crate) test_fn: Arc<Mutex<TestFn>>,
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
        self.actor_id.hash(state)
    }
}
