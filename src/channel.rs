#[cfg(not(feature = "async_channel"))]
mod fc;

#[cfg(feature = "async_channel")]
mod ac {
    use crate::event_loop::Payload;

    pub type Tx<A> = async_channel::Sender<Payload<A>>;
    pub type Rx<A> = async_channel::Receiver<Payload<A>>;
    pub type WeakTx<A> = async_channel::WeakSender<Payload<A>>;

    pub(crate) struct Channel<A> {
        tx: Tx<A>,
        rx: Rx<A>,
    }

    impl<A> Channel<A> {
        const fn new(tx: Tx<A>, rx: Rx<A>) -> Self {
            Channel { tx, rx }
        }
    }

    impl<A> Channel<A>
    where
        for<'a> A: 'a,
    {
        pub fn bounded(buffer: usize) -> Self {
            let (tx, rx) = async_channel::bounded(buffer);

            Self::new(tx, rx)
        }

        pub fn unbounded() -> Self {
            let (tx, rx) = async_channel::unbounded();

            Self::new(tx, rx)
        }

        pub fn break_up(self) -> (Tx<A>, Rx<A>) {
            (self.tx, self.rx)
        }
    }
}

#[cfg(not(feature = "async_channel"))]
pub(crate) use fc::{Channel, Rx, Tx, WeakTx};

#[cfg(feature = "async_channel")]
pub(crate) use ac::{Channel, Rx, Tx, WeakTx};
