use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
};

use futures::{FutureExt as _, channel::oneshot, future::Shared};

use super::ContextID;
use crate::error::Result;

pub type RunningFlag = Arc<AtomicBool>;
pub type RunningFuture = Shared<oneshot::Receiver<()>>;

#[derive(Clone)]
pub(crate) struct Core {
    pub id: ContextID,
    running_flag: RunningFlag,
    stop_future: RunningFuture,
}

impl Core {
    pub fn new(running_flag: RunningFlag, stop_future: RunningFuture) -> Self {
        Self {
            id: ContextID::default(),
            running_flag,
            stop_future,
        }
    }

    /// Returns true if the actor is still running.
    pub fn running(&self) -> bool {
        self.running_flag.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Returns true if the actor has stopped.
    pub fn stopped(&self) -> bool {
        !self.running()
    }
}

impl Future for Core {
    type Output = Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        log::trace!("polling core");
        self.get_mut()
            .stop_future
            .poll_unpin(cx)
            .map(|p| p.map_err(Into::into))
    }
}

pub struct StopNotifier {
    running: RunningFlag,
    notify: Option<oneshot::Sender<()>>,
}

impl StopNotifier {
    pub(crate) const fn new(running: RunningFlag, tx: oneshot::Sender<()>) -> Self {
        Self {
            running,
            notify: Some(tx),
        }
    }

    pub fn notify(mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(tx) = self.notify.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for StopNotifier {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(tx) = self.notify.take() {
            let _ = tx.send(());
        }
    }
}
