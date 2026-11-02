use std::sync::{
    LazyLock,
    atomic::{AtomicU32, Ordering},
};

use crate::Shape;

const NULL_KEY: u32 = u32::MAX;

#[derive(Debug)]
pub struct UntypedKey {
    slot: AtomicU32,
    path: &'static str,
}

impl UntypedKey {
    pub const fn new(path: &'static str) -> Self {
        Self { slot: AtomicU32::new(NULL_KEY), path }
    }

    /// Requires `init_relation_keys()` to have completed. Guaranteed if this
    /// key was reached through an `Ecs`, since `Ecs::new` initializes first.
    #[inline]
    pub(crate) fn slot(&self) -> u32 {
        self.slot.load(Ordering::Relaxed)
    }

    #[inline]
    pub const fn path(&self) -> &'static str {
        self.path
    }
}

impl std::fmt::Display for UntypedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Key#{}({})", self.slot(), self.path())
    }
}

#[linkme::distributed_slice]
pub static STATIC_RELATIONS: [&'static UntypedKey];

pub fn init_relation_keys() -> usize {
    static STATIC_COUNT: LazyLock<usize> = LazyLock::new(|| {
        assert!(STATIC_RELATIONS.len() < NULL_KEY as usize, "too many relation keys");

        let mut keys = STATIC_RELATIONS.iter().collect::<Vec<_>>();

        keys.sort_unstable_by_key(|k| k.path);

        for pair in keys.windows(2) {
            assert!(
                pair[0].path != pair[1].path,
                "duplicate relation path `{}`",
                pair[0].path
            );
        }

        for (i, key) in keys.iter().enumerate() {
            key.slot.store(i as u32, Ordering::Relaxed);
        }

        keys.len()
    });
    *STATIC_COUNT
}

#[inline]
pub fn relation_key_count() -> usize {
    init_relation_keys()
}

#[derive(Debug, Clone, Copy)]
pub struct RelationKey {
    inner: &'static UntypedKey,
    shape: Shape,
}

impl RelationKey {
    #[inline(always)]
    pub const fn new(inner: &'static UntypedKey) -> Self {
        Self::new_with_shape(
            inner,
            Shape::Directed {
                unique_source: false,
                unique_target: false,
                acyclic: false,
                reverse: false,
            },
        )
    }

    #[inline(always)]
    pub const fn new_with_shape(inner: &'static UntypedKey, shape: Shape) -> Self {
        Self { inner, shape }
    }

    #[inline(always)]
    pub const fn untyped(&self) -> &UntypedKey {
        self.inner
    }

    #[inline(always)]
    pub(crate) fn slot(&self) -> u32 {
        self.inner.slot()
    }

    #[inline(always)]
    pub const fn path(&self) -> &'static str {
        self.inner.path()
    }

    #[inline(always)]
    pub const fn shape(&self) -> Shape {
        self.shape
    }
}

impl std::fmt::Display for RelationKey {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.untyped().fmt(f)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unregistered ({path})")]
pub struct UnregisteredKey {
    pub path: &'static str,
}

#[inline]
pub fn unregistered(key: &UntypedKey) -> UnregisteredKey {
    UnregisteredKey { path: key.path() }
}
