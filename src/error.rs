//! Error types for the actor system.
//!

use thiserror::Error;

/// Result type for actor operations.
pub type Result<T> = std::result::Result<T, ActorError>;

// use crate::Actor;

/// Errors produced from within the actor event-loop.
#[derive(Error, Debug, PartialEq)]
#[non_exhaustive]
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

    /// Indicates that a services failed to be registered because an instance of the same type is already registered.
    #[error("Service already registered")]
    ServiceStillRunning,

    /// Indicates that an actor's task took too long to complete.
    #[error("Actor's task took too long to complete")]
    Timeout,
}
