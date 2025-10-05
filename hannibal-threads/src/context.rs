#![allow(unused_imports)]
use std::sync::{Arc, RwLock, mpsc};

use crate::{ActorResult, channel::WeakChanTx, error::ActorError::WriteError, handler::Handler};

pub struct Context<A> {
    pub weak_tx: WeakChanTx<A>,
    // rx: Mutex<Option<mpsc::Receiver<Payload>>>,
}

// impl<A> Default for Context<A> {
//     fn default() -> Self {
//         let (tx, rx) = std::sync::mpsc::channel::<Payload>();
//         Context {
//             weak_tx: Arc::new(tx),
//             rx: Mutex::new(Some(rx)),
//             phantom: std::marker::PhantomData,
//         }
//     }
// }

// impl<A> Context<A> {
//     pub fn take_rx(&mut self) -> Option<mpsc::Receiver<Payload>> {
//         self.rx.lock().ok().and_then(|mut orx| orx.take())
//     }

//     pub fn send<M, H>(&self, msg: M, handler: Arc<RwLock<H>>) -> ActorResult<()>
//     where
//         H: Handler<M> + 'static,
//         M: Send + Sync + 'static,
//     {
//         let handler = handler.clone();
//         self.weak_tx.send(Payload::from(move || {
//             let mut handler = handler.write().map_err(|_| WriteError)?;
//             handler.handle(msg);
//             Ok(())
//         }))?;
//         Ok(())
//     }

//     // TODO: add oneshot to notify Addrs
//     // TODO: mark self as stopped for loop
//     pub fn stop(&self) -> ActorResult<()> {
//         Ok(self.weak_tx.send(Payload::Stop)?)
//     }
// }
