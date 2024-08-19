use super::*;

pub(crate) enum Payload {
    Exec(Box<dyn FnOnce() + Send + 'static>),
    Stop,
}

pub struct EventLoop {
    pub ctx: Context,
}

impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLoop {
    pub fn new() -> Self {
        EventLoop {
            ctx: Context::new(),
        }
    }

    pub fn start<A>(mut self, actor: A) -> Addr<A>
    where
        A: Actor + Send + Sync + 'static,
    {
        let actor = Arc::new(actor);
        self.spawn(actor.clone()).unwrap();
        Addr {
            ctx: Arc::new(Mutex::new(self.ctx)),
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
        let Some(rx) = self.ctx.rx.lock().ok().and_then(|mut orx| orx.take()) else {
            eprintln!("Cannot spawn context");
            return Err(format!("Cannot spawn context"));
        };

        std::thread::spawn((Self::sync_loop(actor, rx)).unwrap());
        Ok(())
    }
}
