// use std::sync::{PoisonError, RwLockWriteGuard};

use thiserror::Error;

use crate::event_loop::Payload;

// use crate::Actor;

#[derive(Error, Debug)]
pub enum ActorError {
    #[error("Actor already stopped")]
    AlreadyStopped,

    #[error("Failed to get lock on actor")]
    WriteError,
    // WriteError(#[from] PoisonError<RwLockWriteGuard<'static, dyn Actor + Send + Sync + 'static>>),
    #[error("Failed to spawn")]
    SpawnError,

    #[error("Failed to send message")]
    SendError(#[from] std::sync::mpsc::SendError<Payload>),
}
