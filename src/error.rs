// use std::sync::{PoisonError, RwLockWriteGuard};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ActorError>;

// use crate::Actor;

#[derive(Error, Debug, PartialEq)]
pub enum ActorError {
    /// The sender failed to send a message on an internal channel.
    #[error("Failed to send message")]
    AsyncSendError(#[from] futures::channel::mpsc::SendError),

    /// The channel that is used to determine that the actor has stopped was closed unexpectedly.
    /// This usually means that the actor task was canceled or panicked.
    #[error("Call got canceled")]
    Canceled(#[from] futures::channel::oneshot::Canceled),

    /// Indicates that the actor has already been stopped by the time the operation was attempted.
    #[error("Actor already stopped")]
    AlreadyStopped,

    #[error("Service not found")]
    ServiceNotFound,
}
