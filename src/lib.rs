//! # Motivation
//!
//! In async Rust, you often find yourself spawning tasks and creating channels to communicate between them.
//! This can become cumbersome in larger projects and complicated when supporting multiple message types.
//! An [`Actor`] *is* a task that can receive and handle [messages](`Message`) and store internal state.
//!
//! ## Example
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
    addr::{
        caller::Caller, sender::Sender, weak_addr::WeakAddr, weak_caller::WeakCaller,
        weak_sender::WeakSender, Addr, Message,
    },
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
