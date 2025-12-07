use std::sync::{LazyLock, atomic::AtomicU64};

static CONTEXT_ID: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContextID(u64);

impl Default for ContextID {
    fn default() -> Self {
        Self(CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl std::fmt::Display for ContextID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[test]
fn ids_go_up() {
    let id1 = ContextID::default();
    let id2 = ContextID::default();
    let id3 = ContextID::default();
    assert!(id1 < id2);
    assert!(id2 < id3);
}
