use std::{pin::Pin, task::Poll};

use futures::FutureExt as _;

use super::{ContextID, RunningFuture};
use crate::error::Result;

#[derive(Clone)]
pub(crate) struct Core {
    pub id: ContextID,
    running: RunningFuture,
}

impl Core {
    pub fn new(running: RunningFuture) -> Self {
        Self {
            id: ContextID::default(),
            running,
        }
    }

    /// Returns true if the actor is still running.
    pub fn running(&self) -> bool {
        self.running.peek().is_none()
    }

    /// Returns true if the actor has stopped.
    pub fn stopped(&self) -> bool {
        self.running.peek().is_some()
    }
}

impl Future for Core {
    type Output = Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        log::trace!("polling core");
        self.get_mut()
            .running
            .poll_unpin(cx)
            .map(|p| p.map_err(Into::into))
    }
}
