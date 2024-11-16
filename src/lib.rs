mod actor;
mod addr;
mod channel;
mod context;
mod environment;
pub mod error;

mod handler;

// TODO: flatten module structure
pub use self::{
    actor::{
        service::{self, Service},
        spawner, Actor, DynResult, RestartableActor,
    },
    addr::{sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender, Addr, Message},
    context::Context,
    handler::{Handler, StreamHandler},
};

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub use actor::build;

pub mod prelude {
    pub use crate::{
        actor::{service::Service, Actor, DynResult},
        addr::{sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender, Addr, Message},
        context::Context,
        handler::{Handler, StreamHandler},
        spawner::{Spawnable, StreamSpawnable},
    };
}
