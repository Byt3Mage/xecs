use std::{
    any::TypeId,
    collections::{HashMap, hash_map::Entry},
    hash::{BuildHasherDefault, Hasher},
};

use crate::memory::RowLayout;

#[derive(Debug, Clone, Copy)]
pub struct TypeMeta {
    pub layout: RowLayout,
    pub type_id: TypeId,
    pub type_name: fn() -> &'static str,
}

impl TypeMeta {
    #[inline]
    pub const fn of<T: Copy + 'static>() -> Self {
        Self {
            layout: RowLayout::of::<T>(),
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>,
        }
    }

    #[inline(always)]
    pub const fn is_zst(&self) -> bool {
        self.layout.size() == 0
    }

    #[inline(always)]
    pub fn name(&self) -> &'static str {
        (self.type_name)()
    }
}

pub trait HasMeta: Copy + 'static {
    const META: &'static TypeMeta;
    const IS_ZST: bool;
}

impl<T: Copy + 'static> HasMeta for T {
    const META: &TypeMeta = &TypeMeta::of::<Self>();
    const IS_ZST: bool = core::mem::size_of::<Self>() == 0;
}

/// Passes a `TypeId`'s own hash bits through unchanged. `TypeId` hashes
/// itself with a single `write_u64`; `write` mixes bytes for any other
/// input.
#[derive(Default)]
struct TypeIdHasher(u64);

impl Hasher for TypeIdHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline(always)]
    fn write_u64(&mut self, n: u64) {
        self.0 = n;
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ (b as u64)).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
}

#[repr(transparent)]
pub(crate) struct TypeIdMap<V>(HashMap<TypeId, V, BuildHasherDefault<TypeIdHasher>>);

impl<V> Default for TypeIdMap<V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<V> TypeIdMap<V> {
    pub(crate) fn new() -> Self {
        Self(HashMap::default())
    }

    #[inline(always)]
    pub(crate) fn get<T: 'static>(&self) -> Option<&V> {
        self.0.get(&TypeId::of::<T>())
    }

    #[inline(always)]
    pub(crate) fn entry<T: 'static>(&mut self) -> Entry<'_, TypeId, V> {
        self.0.entry(TypeId::of::<T>())
    }
}
