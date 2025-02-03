#![deny(clippy::unwrap_used)]

mod actor;
mod addr;
mod channel;
mod context;
mod environment;
pub mod error;

pub use hannibal_derive::message;

#[cfg(any(feature = "tokio", feature = "async-std"))]
mod broker;
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

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub use broker::Broker;

pub mod prelude {
    pub use crate::{
        actor::{service::Service, Actor, DynResult},
        addr::{sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender, Addr, Message},
        context::Context,
        handler::{Handler, StreamHandler},
        message,
        spawner::{Spawnable, StreamSpawnable},
    };
    pub use hannibal_derive::*;
}
