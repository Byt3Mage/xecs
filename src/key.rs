use std::{
    marker::PhantomData,
    sync::{
        LazyLock,
        atomic::{AtomicU32, Ordering},
    },
};

use crate::{Shape, TypeMeta, type_meta::HasMeta};

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

    /// Requires `initialize_keys()` to have completed. Guaranteed if this key was
    /// reached through a `World`, since `World::new` initializes first.
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
pub static STATIC_COMPONENTS: [&'static UntypedKey];

fn init_component_keys() -> usize {
    static STATIC_COUNT: LazyLock<usize> = LazyLock::new(|| {
        assert!(STATIC_COMPONENTS.len() < NULL_KEY as usize, "too many component keys");

        let mut ids = STATIC_COMPONENTS.iter().collect::<Vec<_>>();

        ids.sort_unstable_by_key(|k| k.path);

        for pair in ids.windows(2) {
            assert!(
                pair[0].path != pair[1].path,
                "duplicate component path `{}`",
                pair[0].path
            );
        }

        for (i, key) in ids.iter().enumerate() {
            key.slot.store(i as u32, Ordering::Relaxed);
        }

        ids.len()
    });
    *STATIC_COUNT
}

#[inline]
pub fn component_key_count() -> usize {
    init_component_keys()
}

#[linkme::distributed_slice]
pub static STATIC_RELATIONS: [&'static UntypedKey];

pub fn init_relation_keys() -> usize {
    static STATIC_COUNT: LazyLock<usize> = LazyLock::new(|| {
        assert!(STATIC_RELATIONS.len() < NULL_KEY as usize, "too many relation keys");

        let mut ids = STATIC_RELATIONS.iter().collect::<Vec<_>>();

        ids.sort_unstable_by_key(|k| k.path);

        for pair in ids.windows(2) {
            assert!(
                pair[0].path != pair[1].path,
                "duplicate component path `{}`",
                pair[0].path
            );
        }

        for (i, key) in ids.iter().enumerate() {
            key.slot.store(i as u32, Ordering::Relaxed);
        }

        ids.len()
    });
    *STATIC_COUNT
}

#[inline]
pub fn relation_key_count() -> usize {
    init_relation_keys()
}

pub struct ComponentKey<T: HasMeta> {
    id: UntypedKey,
    marker: PhantomData<fn() -> T>,
}

impl<T: HasMeta> ComponentKey<T> {
    #[inline(always)]
    pub const fn new(path: &'static str) -> Self {
        Self { id: UntypedKey::new(path), marker: PhantomData }
    }

    #[inline(always)]
    pub const fn untyped(&self) -> &UntypedKey {
        &self.id
    }

    #[inline(always)]
    pub(crate) fn slot(&self) -> u32 {
        self.id.slot()
    }

    #[inline(always)]
    pub const fn path(&self) -> &'static str {
        self.id.path()
    }
}

impl<T: HasMeta> std::fmt::Display for ComponentKey<T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.untyped().fmt(f)
    }
}

pub struct RelationKey<T: HasMeta> {
    id: UntypedKey,
    shape: Shape,
    marker: PhantomData<fn() -> T>,
}

impl<T: HasMeta> RelationKey<T> {
    #[inline(always)]
    pub const fn new(path: &'static str) -> Self {
        Self::new_with_shape(
            path,
            Shape::Directed {
                unique_source: false,
                unique_target: false,
                acyclic: false,
                reverse: false,
            },
        )
    }

    #[inline(always)]
    pub const fn new_with_shape(path: &'static str, shape: Shape) -> Self {
        Self {
            id: UntypedKey::new(path),
            shape,
            marker: PhantomData,
        }
    }

    #[inline(always)]
    pub const fn untyped(&self) -> &UntypedKey {
        &self.id
    }

    #[inline(always)]
    pub(crate) fn slot(&self) -> u32 {
        self.id.slot()
    }

    #[inline(always)]
    pub const fn path(&self) -> &'static str {
        self.id.path()
    }

    #[inline(always)]
    pub const fn topo(&self) -> Shape {
        self.shape
    }

    #[inline(always)]
    pub const fn meta(&self) -> &'static TypeMeta {
        T::META
    }
}

impl<T: HasMeta> std::fmt::Display for RelationKey<T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.untyped().fmt(f)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unregistered key: Key#{id}({path})")]
pub struct UnregisteredKey {
    pub id: u32,
    pub path: &'static str,
}

#[inline]
pub fn unregistered(key: &UntypedKey) -> UnregisteredKey {
    UnregisteredKey { id: key.slot(), path: key.path() }
}
