//! # Motivation
//!
//! In async Rust, you often find yourself spawning tasks and creating channels to communicate between them.
//! This can become cumbersome in larger projects and complicated when supporting multiple message types.
//! An [`Actor`] *is* a task that can receive and handle [messages](`Message`) and store internal state.
//!
//!
//! ## Simple Example
//!
//! ```rust
//! # use hannibal::prelude::*;
//! #[derive(Actor)]
//! struct Adder(&'static str);
//!
//! #[message(response = String)]
//! struct Add(i32, i32);
//!
//! impl Handler<Add> for Adder {
//!     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Add) -> String {
//!         format!("{prefix}{result}", prefix = self.0, result = msg.0 + msg.1)
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Spawn the actor and get its address
//! let mut addr = Adder("The Answer is = ").spawn();
//!
//! // Expecting a response
//! let addition = addr.call(Add(1, 2)).await.unwrap();
//!
//! println!("The Actor Calculated: {:?}", addition);
//! # }
//! ```
//!
//! ## Runtime behavior
//! Actors can also be used to handle [Streams](`futures::Stream`),
//! they can be configured to enforce timeouts and use bounded or unbounded channels under the hood.
//! Take a look at [`hannibal::build()`](`build`)
//! to see how to configure an actor's runtime behavior and how to launch them on streams.

#![warn(rustdoc::broken_intra_doc_links)]
// #![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

mod actor;
mod addr;
mod channel;
mod context;
mod environment;
pub mod error;

pub use hannibal_derive::message;

#[cfg(any(feature = "tokio_runtime", feature = "async_runtime"))]
mod broker;
mod handler;

#[cfg(all(feature = "async_runtime", feature = "tokio_runtime"))]
compile_error!("only one runtime featured allowed");

// TODO: flatten module structure
pub use self::{
    actor::{
        Actor, DynResult, RestartableActor,
        service::{self, Service},
        spawner,
    },
    addr::{
        Addr, Message, OwningAddr, caller::Caller, sender::Sender, weak_addr::WeakAddr,
        weak_caller::WeakCaller, weak_sender::WeakSender,
    },
    context::Context,
    handler::{Handler, StreamHandler},
};

#[cfg(any(feature = "tokio_runtime", feature = "async_runtime"))]
pub use actor::build;

#[cfg(any(feature = "tokio_runtime", feature = "async_runtime"))]
pub use broker::Broker;

pub mod prelude {
    //! Re-exports the most commonly used traits and types.
    pub use crate::{
        actor::{Actor, DynResult, service::Service},
        addr::{Addr, Message, sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender},
        context::Context,
        handler::{Handler, StreamHandler},
        message,
        spawner::{Spawnable, StreamSpawnable},
    };
    pub use hannibal_derive::*;
}
