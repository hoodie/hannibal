use hannibal::*;

/// Define `Ping` message
#[message]
struct Ping(usize);

/// Actor
struct MyActor;

/// Declare actor and its context
impl Actor for MyActor {}

/// Handler for `Ping` message
#[async_trait::async_trait]
impl Handler<Ping> for MyActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: Ping) {}
}

#[test]
fn caller_clone() {
    async fn main() -> hannibal::Result<()> {
        // start new actor
        let addr = MyActor.start().await?;

        let caller: Caller<Ping> = addr.caller();
        let caller2 = &Clone::clone(&caller);
        let _caller_is_send: &dyn Send = &Clone::clone(&caller);
        let _caller_is_sync: &dyn Sync = &Clone::clone(&caller);

        caller.call(Ping(10)).await?;
        caller2.call(Ping(10)).await?;

        Ok(())
    }
    hannibal::block_on(main()).unwrap();
}
