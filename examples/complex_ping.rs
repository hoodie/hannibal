use hannibal::*;

/// Define `Ping` message
#[message(result = std::result::Result<Vec<String>, (usize, std::io::Error)>)]
struct ComplexPing(usize);

/// Actor
struct MyActor;

/// Declare actor and its context
impl Actor for MyActor {}

/// Handler for `Ping` message
impl Handler<ComplexPing> for MyActor {
    async fn handle(
        &mut self,
        ctx: &mut Context<Self>,
        ComplexPing(code): ComplexPing,
    ) -> std::result::Result<Vec<String>, (usize, std::io::Error)> {
        if code == 0 {
            Ok(["everything", "is", "alright"]
                .map(ToString::to_string)
                .into())
        } else {
            let error = std::io::Error::other("this was not correct");
            ctx.stop(None);
            Err((code, error))
        }
    }
}

#[hannibal::main]
async fn main() -> Result<()> {
    // start new actor
    let addr = MyActor.start().await?;

    // send message and get future for result
    println!("response to 0: {:?}", addr.call(ComplexPing(0)).await);
    println!("response to 1: {:?}", addr.call(ComplexPing(1)).await);
    println!("response to 0: {:?}", addr.call(ComplexPing(0)).await);

    Ok(())
}
