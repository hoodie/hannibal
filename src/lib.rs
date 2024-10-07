#![allow(clippy::type_complexity)]
#![warn(clippy::doc_markdown)]
#![deny(trivial_numeric_casts, unsafe_code, unused_import_braces)]
#![warn(
    clippy::dbg_macro,
    clippy::doc_markdown,
    clippy::indexing_slicing,
    clippy::redundant_closure_for_method_calls,

    clippy::map_clone,
    clippy::correctness,
    clippy::style,
    clippy::manual_filter_map,
    clippy::useless_format,
    clippy::clone_on_ref_ptr,

    clippy::complexity,
    clippy::too_many_arguments,
    clippy::needless_collect,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::missing_panics_doc,
    clippy::missing_const_for_fn,
    clippy::redundant_clone,

    missing_copy_implementations,
    // missing_debug_implementations,
    // missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
)]

mod actor;
mod addr;
mod channel;
mod context;
mod environment;
pub mod error;
mod payload;

mod handler;

pub use self::{
    actor::{Actor, ActorResult},
    addr::{sender::Sender, weak_caller::WeakCaller, weak_sender::WeakSender, Addr, Message},
    context::Context,
    environment::Environment,
    error::{ActorError, Result},
    handler::Handler,
};
