mod addr;
mod channel;
mod context;
mod event_loop;
mod payload;
mod sender;

mod error;

pub mod actor;
pub mod handler;

pub use addr::Addr;
pub use channel::Channel;
pub use context::Context;
pub use event_loop::EventLoop;
pub use sender::Sender;

pub type ActorResult<T> = std::result::Result<T, error::ActorError>;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub use actor::Actor;
