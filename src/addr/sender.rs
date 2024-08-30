use dyn_clone::DynClone;

use super::{tester::TestFn, ActorId, Message, Result};
use std::hash::{Hash, Hasher};

pub(crate) trait SenderFn<M: Message>: DynClone + 'static + Send + Sync {
    fn send(&self, msg: M) -> Result<()>;
}

impl<F, M> SenderFn<M> for F
where
    F: Fn(M) -> Result<()>,
    F: 'static + Send + Sync + Clone,
    M: Message,
{
    fn send(&self, msg: M) -> Result<()> {
        self(msg)
    }
}

/// Sender of a specific message type
///
/// Like [`Caller<M>`](`super::Caller<M>`), Sender has a weak reference to the recipient of the message type,
/// and so will not prevent an actor from stopping if all [`Addr`](`crate::Addr`)'s have been dropped elsewhere.
/// This allows it to be used in the `send_later` and `send_interval` actor functions,
/// and not keep the actor alive indefinitely even after all references to it have been dropped (unless `ctx.stop()` is called from within)

pub struct Sender<M: Message> {
    /// Id of the corresponding [`Actor<A>`](crate::Actor)
    pub actor_id: ActorId,
    pub(crate) sender_fn: Box<dyn SenderFn<M>>,
    pub(crate) test_fn: Box<dyn TestFn>,
}

impl<M: Message<Result = ()>> Sender<M> {
    pub fn send(&self, msg: M) -> Result<()> {
        self.sender_fn.send(msg)
    }
    pub fn can_upgrade(&self) -> bool {
        self.test_fn.test()
    }
}

impl<M: Message<Result = ()>> PartialEq for Sender<M> {
    fn eq(&self, other: &Self) -> bool {
        self.actor_id == other.actor_id
    }
}

impl<M: Message<Result = ()>> Hash for Sender<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.actor_id.hash(state);
    }
}
