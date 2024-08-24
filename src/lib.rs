//! # Hannibal is a rust actors framework based on async-std
//!
//! ## Documentation
//!
//! * [GitHub repository](https://github.com/hoodie/hannibal)
//! * [Cargo package](https://crates.io/crates/hannibal)
//! * Minimum supported Rust version: 1.56 or later
//!
//! ## Features
//!
//! * Async actors.
//! * Actor communication in a local context.
//! * Using Futures for asynchronous message handling.
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
//!
//! ## References
//!
//! |                                                    |                          |
//! | -------------------------------------------------- | ------------------------ |
//! | [Actix](https://github.com/actix/actix)            | the original inspiration |
//! | [Async-std](https://github.com/async-rs/async-std) | a supported runtime      |
//! | [Tokio](https://tokio.rs/)                         | a supported runtime      |
//! | [Xactor](https://github.com/sunli829/xactor)       | original version of this |

#![feature(async_closure)]

#![allow(clippy::type_complexity)]
#![warn(clippy::doc_markdown)]

mod actor;
mod addr;
mod weak_addr;

mod broker;
mod context;
mod lifecycle;
mod runtime;
mod service;
mod supervisor;

#[cfg(all(feature = "anyhow", feature = "eyre"))]
compile_error!(
    r#"
    features `hannibal/anyhow` and `hannibal/eyre` are mutually exclusive.
    If you are trying to disable anyhow set `default-features = false`.
"#
);

#[cfg(feature = "anyhow")]
pub use anyhow as error;

#[cfg(feature = "eyre")]
pub use eyre as error;

#[cfg_attr(feature = "eyre", doc = "Alias of [`eyre::Result`]")]
#[cfg_attr(not(feature = "eyre"), doc = "Alias of [`anyhow::Result`]")]
pub type Result<T> = error::Result<T>;

#[cfg_attr(feature = "eyre", doc = "Alias of [`eyre::Error`]")]
#[cfg_attr(not(feature = "eyre"), doc = "Alias of [`anyhow::Error`]")]
pub type Error = error::Error;

pub type ActorId = u64;

pub use actor::{Actor, Handler, Message, StreamHandler};

pub use addr::{
    caller::{Caller, WeakCaller},
    sender::{Sender, WeakSender},
    Addr,
};
pub use weak_addr::WeakAddr;

pub use broker::Broker;

pub use context::Context;
pub use hannibal_derive::{main, message};
mod channel;

pub use runtime::{block_on, sleep, spawn, timeout};
pub use service::{LocalService, Service};
pub use supervisor::Supervisor;
