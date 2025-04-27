use hannibal::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Default, Actor, Service)]
struct StorageService {
    storage: HashMap<String, String>,
}

#[message]
struct Store(&'static str, &'static str);

#[message(response = Option<String>)]
struct Retrieve(&'static str);

impl Handler<Store> for StorageService {
    async fn handle(&mut self, _: &mut Context<Self>, Store(key, value): Store) {
        self.storage.insert(key.to_string(), value.to_string());
    }
}

impl Handler<Retrieve> for StorageService {
    async fn handle(&mut self, _: &mut Context<Self>, Retrieve(key): Retrieve) -> Option<String> {
        self.storage.get(key).cloned()
    }
}

#[hannibal::main]
async fn main() {
    env_logger::init();
    StorageService::from_registry()
        .await
        .send(Store("password", "hello world"))
        .await
        .unwrap();

    let result = StorageService::from_registry()
        .await
        .call(Retrieve("password"))
        .await
        .unwrap();

    println!("retrieved: {:?}", result);
}
