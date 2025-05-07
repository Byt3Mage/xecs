use crate::{storage::sparse_set::SparseIndex, utils::NoOpHash};
use std::{collections::HashMap, fmt::Display};

pub type EntityId = u32;

/// Specialized hashmap with optimized no-op hashing for entities.
pub(crate) type EntityMap<V> = HashMap<Entity, V, NoOpHash>;

/// FFI compatible representation of an entity.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Entity(u64);

impl Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entity({}, v{})", self.id(), self.generation())
    }
}

impl Entity {
    pub const NULL: Entity = Entity(0);
    pub const HI_COMPONENT_ID: Entity = Entity(256);

    /// Creates a new `Entity` from raw bits.
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Converts the `Entity` back to raw bits.
    pub const fn to_raw(&self) -> u64 {
        self.0
    }

    /// Returns the ID (lower 32 bits).
    pub const fn id(&self) -> u32 {
        self.0 as u32
    }

    /// Returns the generation (higher 32 bits).
    pub const fn generation(&self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Increments the generation counter (wraps on overflow).
    ///
    /// Allowed to wrap since its highly unlikely that
    /// the same entity will be created and destroyed 4 billion times.
    pub const fn inc_generation(&self) -> Self {
        Self((((self.0 >> 32) + 1) as u64) << 32 | (self.id() as u64))
    }

    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }

    pub const fn is_null(&self) -> bool {
        self.0 == Self::NULL.0
    }
}

impl SparseIndex for Entity {
    #[inline(always)]
    fn to_sparse_index(&self) -> usize {
        self.0 as usize
    }
}
