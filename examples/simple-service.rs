use minibal::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Default)]
struct StorageService {
    storage: HashMap<String, String>,
}

struct Store(&'static str, &'static str);

impl Message for Store {
    type Result = ();
}

struct Retrieve(&'static str);

impl Message for Retrieve {
    type Result = Option<String>;
}

impl Actor for StorageService {}
impl Service for StorageService {}

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

#[tokio::main]
async fn main() {
    StorageService::setup().await.unwrap();
    StorageService::from_registry()
        .await
        .send(Store("password", "hello world"))
        .unwrap();

    let result = StorageService::from_registry()
        .await
        .call(Retrieve("password"))
        .await
        .unwrap();

    println!("retrieved: {:?}", result);
}
