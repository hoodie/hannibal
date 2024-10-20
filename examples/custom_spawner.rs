#[cfg(all(not(feature = "tokio"), not(feature = "async-std")))]
mod custom_spawner {
    use std::{
        future::Future,
        sync::{Arc, LazyLock, Mutex},
    };

    use futures_executor::ThreadPool;

    static POOL: LazyLock<Mutex<ThreadPool>> = LazyLock::new(|| ThreadPool::new().unwrap().into());
    use futures::task::SpawnExt as _;
    use minibal::{
        prelude::*,
        spawn_strategy::{JoinFuture, Joiner, Spawner},
        DynResult,
    };

    pub struct CustomSpawner;

    impl<A: Actor> Spawner<A> for CustomSpawner {
        fn spawn<F>(future: F) -> Box<dyn Joiner<A>>
        where
            F: Future<Output = DynResult<A>> + Send + 'static,
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

    pub struct MyActor;
    impl Actor for MyActor {
        async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult {
            Ok(())
        }

        async fn stopped(&mut self, _ctx: &mut Context<Self>) {
            eprintln!("stopping actor")
        }
    }

    impl Spawnable<CustomSpawner> for MyActor {}
}

fn main() {
    cfg_if::cfg_if! {
       if #[cfg(all(not(feature = "tokio"), not(feature = "async-std")))]
       {
           use custom_spawner::*;
           use minibal::{prelude::Spawnable as _, spawn_strategy::SpawnableWith};
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
        } else {
            panic!("use `--no-default-features`");

        }
    }
}
