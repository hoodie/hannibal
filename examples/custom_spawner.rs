use std::{
    future::Future,
    sync::{Arc, LazyLock, Mutex},
};

use futures::task::SpawnExt as _;
use futures_executor::ThreadPool;
use minibal::{
    prelude::*,
    spawn_strategy::{JoinFuture, Joiner, SpawnableWith, Spawner},
};

// static POOL: std::cell::RefCell<LocalPool>  = LocalPool::new().into();
static POOL: LazyLock<Mutex<ThreadPool>> = LazyLock::new(|| ThreadPool::new().unwrap().into());

struct CustomSpawner;

impl<A: Actor> Spawner<A> for CustomSpawner {
    fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
    where
        F: Future<Output = crate::DynResult<A>> + Send + 'static,
    {
        eprintln!("Spawning actor with custom spawner");
        let handle = Arc::new(async_lock::Mutex::new(Some({
            let pool = POOL.lock().unwrap();
            pool.spawn_with_handle(future)
        })));

        eprintln!("Spawned actor with custom spawner");
        Box::new(move || -> JoinFuture<A> {
            let handle = Arc::clone(&handle);
            Box::pin(async move {
                let mut handle = handle.lock().await.take().and_then(Result::ok);

                if let Some(handle) = handle.take() {
                    // TODO: don't eat the error
                    handle.await.ok()
                } else {
                    None
                }
            })
        })
    }
}

struct MyActor;
impl Actor for MyActor {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult {
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        eprintln!("stopping actor")
    }
}

impl Spawnable<CustomSpawner> for MyActor {}

// #[cfg(all(not(feature = "tokio"), not(feature = "async-std")))]
fn main() {
    color_backtrace::install();
    futures::executor::block_on(async {
        let (mut _addr, _) = MyActor.spawn_with::<CustomSpawner>().unwrap();
        let (mut addr, mut joiner) = MyActor.spawn_and_get_joiner().unwrap();

        addr.stop().unwrap();
        eprintln!("Actor asked to stop");
        addr.await.unwrap();
        joiner.join().await;
        eprintln!("Actor stopped");
    })
}
