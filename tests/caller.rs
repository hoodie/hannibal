use hannibal::*;

#[message(result = "usize")]
struct Count(usize);

#[message]
struct Ping;

#[derive(Default)]
struct CountActor {
    count: usize,
}
impl Actor for CountActor {}

impl Handler<Count> for CountActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, Count(diff): Count) -> usize {
        self.count += diff;
        self.count
    }
}

impl Handler<Ping> for CountActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) {}
}

#[ctor::ctor]
fn init_color_backtrace() {
    color_backtrace::install();
}

#[test]
fn caller_stop() {
    hannibal::block_on(async {
        let addr = CountActor { count: 10 }.start().await.unwrap();

        let caller: Caller<Count> = addr.caller();
        let caller2 = caller.clone();
        let sender: Sender<Ping> = addr.sender();

        assert!(caller.can_upgrade());
        assert!(sender.can_upgrade());

        assert_eq!(caller.call(Count(10)).await.unwrap(), 20);
        assert_eq!(caller2.call(Count(10)).await.unwrap(), 30);
        assert!(sender.send(Ping).is_ok());

        std::mem::drop(addr);

        assert_eq!(caller.can_upgrade(), false);
        assert_eq!(caller2.can_upgrade(), false);
        assert_eq!(sender.can_upgrade(), false);

        assert!(caller.call(Count(10)).await.is_err());
        assert!(caller2.call(Count(10)).await.is_err());
        assert!(sender.send(Ping).is_err());
    });
}

#[test]
fn caller_clone() {
    async fn main() -> hannibal::Result<()> {
        // start new actor
        let addr = CountActor::default().start().await?;

        let caller: Caller<Ping> = addr.caller();
        let caller2 = &Clone::clone(&caller);
        let _caller_is_send: &dyn Send = &Clone::clone(&caller);
        let _caller_is_sync: &dyn Sync = &Clone::clone(&caller);

        caller.call(Ping).await?;
        caller2.call(Ping).await?;

        Ok(())
    }
    hannibal::block_on(main()).unwrap();
}
