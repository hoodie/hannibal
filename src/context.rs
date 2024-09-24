use std::sync::{mpsc, Arc, Mutex, RwLock};

use crate::{
    channel::ChannelWrapper, error::ActorError::WriteError, event_loop::Payload, Actor,
    ActorResult, Handler,
};

pub type RunningFuture = futures::future::Shared<futures::channel::oneshot::Receiver<()>>;

pub struct Context<A>
{
    weak_tx: crate::channel::WeakChanTx<A>,
    pub(crate) rx_exit: Option<RunningFuture>,
}

impl<A> Context<A> where A: Actor {

}

impl<A> Context<A>
where
    A: Actor,
{
    pub(crate) fn new(rx_exit: Option<RunningFuture>, channel: &ChannelWrapper<A>) -> Self {
        Self {
            weak_tx: channel.weak_tx(),
            rx_exit,
        }
    }
    pub fn take_rx(&mut self) -> Option<mpsc::Receiver<Payload<A>>> {
        self.rx.lock().ok().and_then(|mut orx| orx.take())
    }

    pub fn send<M, H>(&self, msg: M, handler: Arc<RwLock<H>>) -> ActorResult<()>
    where
        H: Handler<M> + 'static,
        M: Send + Sync + 'static,
    {
        let handler = handler.clone();
        self.tx.send(Payload::from(move || {
            let mut handler = handler.write().map_err(|_| WriteError)?;
            handler.handle(msg);
            Ok(())
        }))?;
        Ok(())
    }

    // TODO: add oneshot to notify Addrs
    // TODO: mark self as stopped for loop
    pub fn stop(&self) -> ActorResult<()> {
        Ok(self.tx.send(Payload::Stop)?)
    }
}

// impl<A> Context<A>
// where
//     A: Actor,
// {
//     pub fn weak_sender<M>(&self) -> WeakSender<M>
//     where
//         A: Actor + Handler<M>,
//         M: Message<Result = ()>,
//     {
//         WeakSender::from_weak_tx(self.actor_id, self.weak_tx.clone())
//     }

//     pub fn weak_caller<M>(&self) -> WeakCaller<M>
//     where
//         A: Actor + Handler<M>,
//         M: Message,
//     {
//         WeakCaller::from_weak_tx(self.actor_id, self.weak_tx.clone())
//     }

// }
