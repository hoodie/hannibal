use fnv::FnvHasher;
use futures::lock::Mutex;
use once_cell::sync::OnceCell;
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    hash::BuildHasherDefault,
};

use crate::{error::Result, lifecycle::LifeCycle, Actor, Addr};

#[async_trait::async_trait]
pub trait Service: Actor + Default {
    async fn from_registry() -> Result<Addr<Self>> {
        static REGISTRY: OnceCell<
            Mutex<HashMap<TypeId, Box<dyn Any + Send>, BuildHasherDefault<FnvHasher>>>,
        > = OnceCell::new();
        let registry = REGISTRY.get_or_init(Default::default);
        let mut registry = registry.lock().await;

        match registry.get_mut(&TypeId::of::<Self>()) {
            Some(addr) => Ok(addr.downcast_ref::<Addr<Self>>().unwrap().clone()),
            None => {
                let life_cycle = LifeCycle::new();

                registry.insert(TypeId::of::<Self>(), Box::new(life_cycle.address()));
                drop(registry);

                life_cycle.start_actor(Self::default()).await
            }
        }
    }
}

thread_local! {
    static LOCAL_REGISTRY: RefCell<HashMap<TypeId, Box<dyn Any + Send>, BuildHasherDefault<FnvHasher>>> = Default::default();
}

#[async_trait::async_trait]
pub trait LocalService: Actor + Default {
    async fn from_registry() -> Result<Addr<Self>> {
        let res = LOCAL_REGISTRY.with(|registry| {
            registry
                .borrow_mut()
                .get_mut(&TypeId::of::<Self>())
                .map(|addr| addr.downcast_ref::<Addr<Self>>().unwrap().clone())
        });
        match res {
            Some(addr) => Ok(addr),
            None => {
                let addr = LifeCycle::new().start_actor(Self::default()).await?;
                LOCAL_REGISTRY.with(|registry| {
                    registry
                        .borrow_mut()
                        .insert(TypeId::of::<Self>(), Box::new(addr.clone()));
                });
                Ok(addr)
            }
        }
    }
}
