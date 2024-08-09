use hannibal::*;

/// Define `Ping` message
#[message(result = isize)]
#[derive(Debug, Clone, Copy)]
struct Compute(u8);

/// Actor
#[derive(Debug, Default)]
struct IncActor {
    tally: usize,
}
impl Actor for IncActor {}

impl Handler<Compute> for IncActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Compute) -> isize {
        self.tally += msg.0 as usize;
        self.tally as isize
    }
}

#[derive(Debug, Default)]
struct DecActor {
    tally: isize,
}

impl Actor for DecActor {}

impl Handler<Compute> for DecActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Compute) -> isize {
        self.tally -= msg.0 as isize;
        self.tally
    }
}

struct LoadBallancer<T: Message, const N: usize> {
    callers: [Caller<T>; N],
}

impl<T: Message + Copy, const N :usize> LoadBallancer<T, N> {
    async fn call(&self, msg: T) -> Result<()> {
        for caller in &self.callers {
            caller.call(msg).await?;
        }
        Ok(())
    }
}

#[hannibal::main]
async fn main() -> Result<()> {
    let inc1 = IncActor::start_default().await?.caller::<Compute>();
    let dec1 = DecActor::start_default().await?.caller::<Compute>();

    let container = LoadBallancer {
        callers: [inc1, dec1],
    };

    container.call(Compute(1)).await?;
    container.call(Compute(1)).await?;
    container.call(Compute(1)).await?;
    container.call(Compute(1)).await?;

    Ok(())
}
