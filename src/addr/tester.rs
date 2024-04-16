use dyn_clone::DynClone;

pub(crate) trait TestFn: DynClone + 'static + Send + Sync {
    fn test(&self) -> bool;
}

impl<F> TestFn for F
where
    F: Fn() -> bool,
    F: DynClone + 'static + Send + Sync,
{
    fn test(&self) -> bool {
        self()
    }
}
