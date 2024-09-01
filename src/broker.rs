use crate::{addr::WeakSender, Actor, Addr, Context, Handler, Message, Result, Service};
use fnv::FnvHasher;
use std::{any::Any, collections::HashMap, hash::BuildHasherDefault, marker::PhantomData};

type SubscriptionId = u64;

pub(crate) struct Subscribe<M: Message<Result = ()>> {
    pub(crate) id: SubscriptionId,
    pub(crate) sender: WeakSender<M>,
}

impl<M: Message<Result = ()>> Message for Subscribe<M> {
    type Result = ();
}

pub(crate) struct Unsubscribe {
    pub(crate) id: SubscriptionId,
}

impl Message for Unsubscribe {
    type Result = ();
}

struct Publish<M: Message<Result = ()> + Clone>(M);

impl<M: Message<Result = ()> + Clone> Message for Publish<M> {
    type Result = ();
}

/// Message broker is used to support publishing and subscribing to messages.
///
/// # Examples
///
/// ```rust
/// use hannibal::*;
/// use std::time::Duration;
///
/// #[message]
/// #[derive(Clone)]
/// struct MyMsg(&'static str);
///
/// #[message(result = String)]
/// struct GetValue;
///
/// #[derive(Default)]
/// struct MyActor(String);
///
/// impl Actor for MyActor {
///     async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()>  {
///         ctx.subscribe::<MyMsg>().await;
///         Ok(())
///     }
/// }
///
/// impl Handler<MyMsg> for MyActor {
///     async fn handle(&mut self, _ctx: &mut Context<Self>, msg: MyMsg) {
///         self.0 += msg.0;
///     }
/// }
///
/// impl Handler<GetValue> for MyActor {
///     async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: GetValue) -> String {
///         self.0.clone()
///     }
/// }
///
/// #[hannibal::main]
/// async fn main() -> Result<()> {
///     let mut addr1 = MyActor::start_default().await?;
///     let mut addr2 = MyActor::start_default().await?;
///
///     Broker::from_registry().await?.publish(MyMsg("a"));
///     Broker::from_registry().await?.publish(MyMsg("b"));
///
///     sleep(Duration::from_secs(1)).await; // Wait for the messages
///
///     assert_eq!(addr1.call(GetValue).await?, "ab");
///     assert_eq!(addr2.call(GetValue).await?, "ab");
///     Ok(())
/// }
/// ```
pub struct Broker<M: Message<Result = ()>> {
    subscribes: HashMap<SubscriptionId, Box<dyn Any + Send>, BuildHasherDefault<FnvHasher>>,
    mark: PhantomData<M>,
}

impl<M: Message<Result = ()>> Default for Broker<M> {
    fn default() -> Self {
        Self {
            subscribes: HashMap::default(),
            mark: PhantomData,
        }
    }
}

impl<M: Message<Result = ()>> Actor for Broker<M> {}

impl<M: Message<Result = ()>> Service for Broker<M> {}

impl<M: Message<Result = ()>> Handler<Subscribe<M>> for Broker<M> {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Subscribe<M>) {
        self.subscribes.insert(msg.id, Box::new(msg.sender));
    }
}

impl<M: Message<Result = ()>> Handler<Unsubscribe> for Broker<M> {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Unsubscribe) {
        self.subscribes.remove(&msg.id);
    }
}

impl<M: Message<Result = ()> + Clone> Handler<Publish<M>> for Broker<M> {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Publish<M>) {
        for sender in self.subscribes.values_mut() {
            if let Some(Err(err)) = sender
                .downcast_mut::<WeakSender<M>>()
                .map(|s| s.try_send(msg.0.clone()))
            {
                // TODO: log the error
                eprintln!("Failed to send message: {:?}", err)
            }
        }
    }
}

impl<M: Message<Result = ()> + Clone> Addr<Broker<M>> {
    /// Publishes a message of the specified type.
    pub fn publish(&mut self, msg: M) -> Result<()> {
        self.send(Publish(msg))
    }
}
