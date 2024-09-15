use thiserror::Error;

/// A type alias for `Result<T, Error>`.
pub type Result<M> = std::result::Result<M, Error>;

/// Error type for things that might go wrong in the actor system.
#[derive(Error, Debug)]
pub enum Error {

    /// Indicates that an error occurred while trying to start an actor.
    #[error("Failed to start actor")]
    StartFailed,

    /// Indicates that the actor has already been stopped by the time the operation was attempted.
    #[error("Actor already stopped")]
    AlreadyStopped,

    /// The sender failed to send a message on an internal channel.
    #[error("Failed to send message")]
    AsyncSendError(#[from] futures::channel::mpsc::SendError),

    /// The channel that is used to determine that the actor has stopped was closed unexpectedly.
    /// This usually means that the actor task was canceled or panicked.
    #[error("Call got canceled")]
    Canceled(#[from] futures::channel::oneshot::Canceled),
}

impl<M> From<Error> for Result<M> {
    fn from(val: Error) -> Self {
        Err(val)
    }
}
