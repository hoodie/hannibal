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
        build,
        service::{self, Service},
        spawn_strategy, Actor, DynResult, RestartableActor,
    },
    addr::{sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender, Addr, Message},
    context::Context,
    handler::{Handler, StreamHandler},
};

pub mod prelude {
    pub use crate::{
        actor::{service::Service, Actor, DynResult},
        addr::{sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender, Addr, Message},
        context::Context,
        handler::{Handler, StreamHandler},
        spawn_strategy::{Spawnable, StreamSpawnable},
    };
}
