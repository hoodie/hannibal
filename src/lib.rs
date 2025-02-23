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
//! struct MyActor(&'static str);
//!
//! #[message]
//! struct Greet(&'static str);
//!
//! #[message(response = i32)]
//! struct Add(i32, i32);
//!
//! impl Handler<Greet> for MyActor {
//!     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Greet) {
//!         println!(
//!             "[Actor {me}] Hello {you}, my name is {me}",
//!             me = self.0,
//!             you = msg.0,
//!         );
//!     }
//! }
//!
//! impl Handler<Add> for MyActor {
//!     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Add) -> i32 {
//!         msg.0 + msg.1
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     // Spawn the actor and get its address
//!     let mut addr = MyActor("Caesar").spawn();
//!
//!     // Send a message without a response
//!     addr.send(Greet("Hannibal")).await.unwrap();
//!
//!     // Expecting a response
//!     let addition = addr.call(Add(1, 2)).await.unwrap();
//!
//!     println!("The Actor Calculated: {:?}", addition);
//! }
//! ```
//!
//! ## Runtime behavior
//! Actors can also be used to handle [Streams](`futures::Stream`),
//! they can be configured to enfore timeouts and use bounded or unbounded channels under the hood.
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

#[cfg(any(feature = "tokio", feature = "async-std"))]
mod broker;
mod handler;

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

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub use actor::build;

#[cfg(any(feature = "tokio", feature = "async-std"))]
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
