//! # Hannibal Actor framework.
//!
//! Hannibal is similar to actix but uses futures and async/await.
//!
//! ## Features
//!
//! * Async actors.
//! * Actor communication in a local context.
//! * Using Futures for asynchronous message handling.
//! * Weak and strong references to actors via `Addr`, `Sender` and `Caller`.
//! * Brokers and Services.
//! * Typed messages (No `Any` type). Generic messages are allowed.
//!
//! ## Examples
//!
//! ```rust
//! use hannibal::*;
//!
//! #[message(result = String)]
//! struct ToUppercase(String);
//!
//! struct MyActor;
//!
//! impl Actor for MyActor {}
//!
//! impl Handler<ToUppercase> for MyActor {
//!     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: ToUppercase) -> String {
//!         msg.0.to_uppercase()
//!     }
//! }
//!
//! #[hannibal::main]
//! async fn main() -> Result<()> {
//!     // Start actor and get its address
//!     let mut addr = MyActor.start().await?;
//!
//!     // Send message `ToUppercase` to actor via addr
//!     let res = addr.call(ToUppercase("lowercase".to_string())).await?;
//!     assert_eq!(res, "LOWERCASE");
//!     Ok(())
//! }
//! ```

#![allow(clippy::type_complexity)]
#![warn(clippy::doc_markdown)]
#![deny(trivial_numeric_casts, unsafe_code, unused_import_braces)]

mod actor;
mod addr;
mod error;
mod weak_addr;

mod broker;
mod context;
mod lifecycle;
mod runtime;
mod service;
mod supervisor;

pub use error::{Error, Result};

pub type ActorId = u64;

pub use actor::{Actor, Handler, Message, StreamHandler};

pub use addr::{Addr, Caller, Sender, StopReason, WeakCaller, WeakSender};
pub use weak_addr::WeakAddr;

pub use broker::Broker;

pub use context::Context;
pub use hannibal_derive::{main, message};
mod channel;

pub use runtime::{block_on, sleep, spawn, timeout};
pub use service::{LocalService, Service};
pub use supervisor::Supervisor;
