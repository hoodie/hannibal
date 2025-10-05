use crate::{
    ActorResult, Addr, Context,
    actor::Actor,
    error::ActorError::{SpawnError, WriteError},
};
use std::sync::{Arc, RwLock, mpsc};

type Exec = dyn FnOnce() -> ActorResult<()> + Send + Sync + 'static;

pub enum Payload {
    Exec(Box<Exec>),
    Stop,
}

impl<F> From<F> for Payload
where
    F: FnOnce() -> ActorResult<()> + Send + Sync + 'static,
{
    fn from(f: F) -> Self {
        Payload::Exec(Box::new(f))
    }
}

type BoxedActor = Arc<RwLock<dyn Actor + Send + Sync + 'static>>;

impl Actor for BoxedActor {
    fn started(&mut self) -> ActorResult<()> {
        self.as_ref().write().map_err(|_| WriteError)?.started()
    }

    fn stopped(&mut self) -> ActorResult<()> {
        self.as_ref().write().map_err(|_| WriteError)?.stopped()
    }
}

#[derive(Default)]
pub struct EventLoop<A> {
    pub ctx: Context<A>,
}

impl EventLoop {
    pub fn start<A>(mut self, actor: A) -> ActorResult<Addr<A>>
    where
        A: Actor + Send + Sync + 'static,
    {
        let actor = Arc::new(RwLock::new(actor));
        self.spawn(actor.clone())?;

        Ok(Addr {
            ctx: Arc::new(self.ctx),
            actor,
        })
    }

    pub(crate) fn sync_loop(
        mut actor: BoxedActor,
        rx: mpsc::Receiver<Payload>,
    ) -> impl FnOnce() -> ActorResult<()> {
        move || {
            actor.started()?;
            let receiving = rx.iter();
            for payload in receiving {
                match payload {
                    Payload::Exec(exec) => exec()?,
                    Payload::Stop => break,
                }
            }
            actor.stopped()?;
            Ok(())
        }
    }

    pub fn spawn(&mut self, actor: BoxedActor) -> ActorResult<()> {
        let Some(rx) = self.ctx.take_rx() else {
            eprintln!("Cannot spawn context");
            return Err(SpawnError);
        };

        std::thread::spawn(Self::sync_loop(actor, rx));
        Ok(())
    }
}
