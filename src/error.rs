use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

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

impl<T> From<Error> for Result<T> {
    fn from(val: Error) -> Self {
        Err(val)
    }
}
