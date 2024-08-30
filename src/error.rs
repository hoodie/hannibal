use thiserror::Error;

pub type Result<M> = std::result::Result<M, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Actor already stopped")]
    AlreadyStopped,

    #[error("Failed to get lock on actor")]
    WriteError,

    #[error("Failed to spawn")]
    SpawnError,

    #[error("Failed to send message")]
    AsyncSendError(#[from] futures::channel::mpsc::SendError),

    #[error("Call got canceled")]
    Canceled(#[from] futures::channel::oneshot::Canceled),
}

impl<M> From<Error> for Result<M> {
    fn from(val: Error) -> Self {
        Err(val)
    }
}
