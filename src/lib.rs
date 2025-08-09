//! # Motivation
//!
//! In async Rust, you often find yourself spawning tasks and creating channels to communicate between them.
//! This can become cumbersome in larger projects and complicated when supporting multiple message types.
//! An [`Actor`] *is* a task that can receive and handle [messages](`Message`) and store internal state.
//!
//!
//! ## Simple Example
//!
//! You can send messages to an actor without expecting a response.
//!
//! ```rust
//! # use hannibal::prelude::*;
//! #[derive(Actor)]
//! struct Greeter(&'static str);
//!
//! #[message]
//! struct Greet(&'static str);
//!
//! impl Handler<Greet> for Greeter {
//!     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Greet) {
//!         println!(
//!             "[Actor {me}] Hello {you}, my name is {me}",
//!             me = self.0,
//!             you = msg.0,
//!         );
//!     }
//! }
//!
//! # #[hannibal::main]
//! # async fn main() {
//! let mut addr = Greeter("Caesar").spawn();
//!
//! addr.send(Greet("Hannibal")).await.unwrap();
//! # }
//! ```
//!
//! You can also call the actor and get a response.
//!
//! ```rust
//! # use hannibal::prelude::*;
//! #[derive(Actor)]
//! struct Calculator();
//!
//! #[message(response = i32)]
//! struct Add(i32, i32);
//!
//! impl Handler<Add> for Calculator {
//!     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Add) -> i32 {
//!         msg.0 + msg.1
//!     }
//! }
//!
//! # #[hannibal::main]
//! # async fn main() {
//! let mut addr = Calculator().spawn();
//!
//! let addition = addr.call(Add(1, 2)).await;
//!
//! println!("The Actor Calculated: {:?}", addition);
//! # }
//! ```
//!
//! ## Runtime behavior
//! Actors can also be used to handle [Streams](`futures::Stream`) by implementing [`StreamHandler`],
//! they can be configured to enforce timeouts and use bounded or unbounded channels under the hood.
//! Take a look at [`hannibal::build()`](`build`)
//! to see how to configure an actor's runtime behavior and how to launch them on streams.

#![warn(rustdoc::broken_intra_doc_links, missing_docs)]
#![deny(clippy::unwrap_used)]

mod actor;
mod addr;
mod channel;
mod context;
mod environment;
pub mod error;

pub use hannibal_derive::{main, message};

mod broker;
mod handler;

pub mod runtime;

// TODO: flatten module structure
pub use self::{
    actor::{
        Actor, DynResult, RestartableActor,
        service::{self, Service},
        spawnable,
    },
    addr::{
        Addr, Message, OwningAddr, caller::Caller, sender::Sender, weak_addr::WeakAddr,
        weak_caller::WeakCaller, weak_sender::WeakSender,
    },
    context::{Context, TaskHandle},
    handler::{Handler, StreamHandler},
};

pub use actor::build;

pub use broker::Broker;

pub mod prelude {
    //! Re-exports the most commonly used traits and types.
    pub use crate::{
        actor::{Actor, DynResult, service::Service},
        addr::{Addr, Message, sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender},
        context::Context,
        handler::{Handler, StreamHandler},
        main, message,
        spawnable::{DefaultSpawnable, Spawnable, StreamSpawnable},
    };
    pub use hannibal_derive::*;
}
