use std::sync::{LazyLock, atomic::AtomicU64};

static TASK_ID: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskID(u64);

impl Default for TaskID {
    fn default() -> Self {
        Self(TASK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

#[test]
fn ids_go_up() {
    let id1 = TaskID::default();
    let id2 = TaskID::default();
    let id3 = TaskID::default();
    assert!(id1 < id2);
    assert!(id2 < id3);
}
