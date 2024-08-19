use event_loop::Payload;

use super::*;

pub struct Sender<M> {
    tx: Arc<mpsc::Sender<Payload>>,
    actor: Arc<dyn Handler<M>>,
    marker: PhantomData<M>,
}

impl<M, A> From<Addr<A>> for Sender<M>
where
    A: Handler<M> + 'static,
{
    fn from(Addr { ctx, actor }: Addr<A>) -> Self {
        Sender {
            tx: ctx.lock().unwrap().tx.clone(),
            actor: actor.clone(),
            marker: PhantomData,
        }
    }
}

impl<M> Sender<M> {
    pub fn send(&self, msg: M)
    where
        M: Send + 'static,
    {
        let actor = self.actor.clone();
        self.tx.send(Payload::Exec(Box::new(move || actor.handle(msg)))).unwrap()
    }

    pub fn downgrade(&self) -> WeakSender<M> {
        WeakSender {
            tx: Arc::downgrade(&self.tx),
            actor: Arc::downgrade(&self.actor),
            marker: PhantomData,
        }
    }
}

pub struct WeakSender<M> {
    tx: Weak<mpsc::Sender<Payload>>,
    actor: Weak<dyn Handler<M>>,
    marker: PhantomData<M>,
}

impl<M> WeakSender<M> {
    pub fn try_send(&self, msg: M) -> bool
    where
        M: Send + 'static,
    {
        if let Some((tx, actor)) = self.tx.upgrade().zip(self.actor.upgrade()) {
            tx.send(Payload::Exec(Box::new(move || actor.handle(msg)))).unwrap();
            true
        } else {
            eprintln!("Actor is dead");
            false
        }
    }
}
