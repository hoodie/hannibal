use crate::{Actor, Addr, Context};
use std::sync::{mpsc, Arc};

pub enum Payload {
    Exec(Box<dyn FnOnce() + Send + 'static>),
    Stop,
}

#[derive(Default)]
pub struct EventLoop {
    pub ctx: Context,
}

impl EventLoop {
    pub fn start<A>(mut self, actor: A) -> Addr<A>
    where
        A: Actor + Send + Sync + 'static,
    {
        let actor = Arc::new(actor);
        self.spawn(actor.clone()).unwrap();
        Addr {
            ctx: Arc::new(self.ctx),
            actor,
        }
    }

    pub(crate) fn sync_loop(
        actor: impl Actor + Send + Sync,
        rx: mpsc::Receiver<Payload>,
    ) -> Result<impl FnOnce(), String> {
        Ok(move || {
            actor.started();
            let receiving = rx.iter();
            for payload in receiving {
                match payload {
                    Payload::Exec(exec) => exec(),
                    Payload::Stop => break,
                }
            }
            actor.stopped();
        })
    }

    pub fn spawn(&mut self, actor: impl Actor + Send + Sync + 'static) -> Result<(), String> {
        let Some(rx) = self.ctx.take_rx() else {
            eprintln!("Cannot spawn context");
            return Err("Cannot spawn context".to_string());
        };

        std::thread::spawn((Self::sync_loop(actor, rx)).unwrap());
        Ok(())
    }
}
