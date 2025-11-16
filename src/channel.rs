use crate::event_loop::Payload;

pub type PayloadTx<A> = flume::Sender<Payload<A>>;
pub type PayloadRx<A> = flume::Receiver<Payload<A>>;

pub type WeakPayloadTx<A> = flume::WeakSender<Payload<A>>;

pub(crate) struct Channel<A> {
    pub tx: PayloadTx<A>,
    pub rx: PayloadRx<A>,
}

impl<A> Channel<A> {
    const fn new(tx: PayloadTx<A>, rx: PayloadRx<A>) -> Self {
        Channel { tx, rx }
    }
}

impl<A> Channel<A>
where
    for<'a> A: 'a,
{
    pub fn bounded(buffer: usize) -> Self {
        let (tx, rx) = flume::bounded(buffer);

        Self::new(tx, rx)
    }

    pub fn unbounded() -> Self {
        let (tx, rx) = flume::unbounded();

        Self::new(tx, rx)
    }
}
