#![allow(unused_imports)]
use crate::{
    ActorResult, Addr, Context,
    actor::Actor,
    channel::{Channel, PayloadStream},
    error::ActorError::SpawnError,
};
use std::sync::{Arc, RwLock, mpsc};

// impl<F, A> From<F> for Payload<A>
// where
//     F: FnOnce() -> ActorResult<()> + Send + Sync + 'static,
// {
//     fn from(f: F) -> Self {
//         Payload::Exec(Box::new(f))
//     }
// }

pub struct EventLoop<A> {
    pub ctx: Context<A>,
    addr: Addr<A>,
    payload_stream: PayloadStream<A>,
}

impl<A: Actor> EventLoop<A> {
    pub(crate) fn from_channel(channel: Channel<A>) -> Self {
        // let (tx_running, rx_running) = oneshot::channel::<()>();
        let ctx = Context {
            // id: Default::default(),
            weak_tx: channel.weak_tx(),
            // weak_force_tx: channel.weak_force_tx(),
        };
        let (payload_tx, payload_stream) = channel.break_up();
        // let stop = StopNotifier(tx_running);

        let addr = Addr {
            payload_tx,
            // context_id: ctx.id,
            // payload_force_tx,
            // payload_tx,
            // running: ctx.running.clone(),
        };
        EventLoop {
            ctx,
            addr,
            // stop,
            payload_stream,
            // config: Default::default(),
            // phantom: PhantomData,
        }
    }
    // pub fn start(mut self, actor: A) -> ActorResult<Addr<A>>
    // where
    //     A: Actor + Send + Sync + 'static,
    // {
    //     let actor = Arc::new(RwLock::new(actor));
    //     self.spawn(actor.clone())?;

    //     Ok(Addr {
    //         ctx: Arc::new(self.ctx),
    //         actor,
    //     })
    // }

    // pub(crate) fn sync_loop(
    //     mut actor: BoxedActor,
    //     rx: mpsc::Receiver<Payload>,
    // ) -> impl FnOnce() -> ActorResult<()> {
    //     move || {
    //         actor.started()?;
    //         let receiving = rx.iter();
    //         for payload in receiving {
    //             match payload {
    //                 Payload::Exec(exec) => exec()?,
    //                 Payload::Stop => break,
    //             }
    //         }
    //         actor.stopped()?;
    //         Ok(())
    //     }
    // }

    // pub fn spawn(&mut self, actor: BoxedActor) -> ActorResult<()> {
    //     let Some(rx) = self.ctx.take_rx() else {
    //         eprintln!("Cannot spawn context");
    //         return Err(SpawnError);
    //     };

    //     std::thread::spawn(Self::sync_loop(actor, rx));
    //     Ok(())
    // }
}
