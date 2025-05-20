pub mod entity_view;
pub(crate) mod id_index;

use crate::{
    storage::sparse_set::{SparseIndex, SparseSet},
    utils::NoOpHash,
};
use std::{collections::HashMap, fmt::Display};

/// FFI compatible representation of an id.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(u64);

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_entity() {
            let idx = self.index();
            let ver = self.generation();
            write!(f, "Entity({idx}, v{ver})")
        } else {
            let rel = self.pair_rel();
            let tgt = self.pair_tgt();
            write!(f, "Pair(rel: {rel}, tgt: {tgt})")
        }
    }
}

impl Id {
    // Id Flags
    pub const PAIR_FLAG: u64 = 1u64 << 63;
    pub const MAX_TGT_ID: u64 = 0x7FFF_FFFF;

    /// Built-in entities
    pub const NULL: Id = Id(0);
    pub const WILDCARD: Id = Id(1);

    /// Creates a new `Entity` from raw bits.
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Converts the `Entity` back to raw bits.
    pub const fn to_raw(&self) -> u64 {
        self.0
    }

    /// Returns the ID (lower 32 bits).
    #[inline]
    pub const fn index(&self) -> u32 {
        self.0 as u32
    }

    /// Returns the generation (higher 32 bits).
    pub const fn generation(&self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Increments the generation counter (wraps on overflow).
    pub(crate) const fn inc_generation(&self) -> Self {
        Self((((self.0 >> 32).wrapping_add(1) as u64) << 32) | (self.index() as u64))
    }

    #[inline(always)]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }

    pub const fn is_null(&self) -> bool {
        self.0 == Self::NULL.0
    }

    pub const fn is_wildcard(&self) -> bool {
        if self.0 == Id::WILDCARD.0 {
            return true;
        }

        if self.is_pair() {
            let rel = self.pair_rel();
            let tgt = self.pair_tgt();
            rel.0 == Id::WILDCARD.0 || tgt.0 == Id::WILDCARD.0
        } else {
            false
        }
    }

    /// Checks if the id is an entity.
    pub const fn is_entity(&self) -> bool {
        (self.0 >> 63) & 1 == 0
    }

    /// Checks if the id is a pair.
    pub const fn is_pair(&self) -> bool {
        (self.0 >> 63) & 1 == 1
    }

    pub const fn pair_rel(&self) -> Self {
        Self((self.0 >> 32) & Self::MAX_TGT_ID)
    }

    pub const fn pair_tgt(&self) -> Self {
        Self((self.0 as u32) as u64)
    }

    pub const fn from_parts(lo: u32, hi: u32) -> Self {
        Self::from_raw(((hi as u64) << 32) | lo as u64)
    }
}

#[inline(always)]
pub fn pair(rel: Id, tgt: Id) -> Id {
    // TODO: consider adding this back
    /*assert!(
        (rel <= Id::MAX_TGT_ID as u32),
        "pair relationship must not exceed 31 bits"
    );*/

    Id((tgt.index() as u64) | ((rel.index() as u64) << 32) | Id::PAIR_FLAG)
}

impl From<(Id, Id)> for Id {
    fn from((rel, tgt): (Id, Id)) -> Self {
        pair(rel, tgt)
    }
}

impl SparseIndex for Id {
    fn to_sparse_index(&self) -> usize {
        self.0 as usize
    }
}

pub struct IdMap<V> {
    ids: SparseSet<Id, V>,
    pairs: HashMap<Id, V, NoOpHash>,
}

impl<V> IdMap<V> {
    pub fn new() -> Self {
        Self {
            ids: SparseSet::new(),
            pairs: HashMap::default(),
        }
    }

    pub fn insert(&mut self, id: Id, val: V) -> Option<V> {
        if id.is_entity() {
            self.ids.insert(id, val)
        } else {
            self.pairs.insert(id, val)
        }
    }

    #[inline]
    pub fn contains(&mut self, id: Id) -> bool {
        if id.is_entity() {
            self.ids.contains_key(&id)
        } else {
            self.pairs.contains_key(&id)
        }
    }

    #[inline]
    pub fn get(&self, id: Id) -> Option<&V> {
        if id.is_entity() {
            self.ids.get(&id)
        } else {
            self.pairs.get(&id)
        }
    }

    #[inline]
    pub fn get_mut(&mut self, id: Id) -> Option<&mut V> {
        if id.is_entity() {
            self.ids.get_mut(&id)
        } else {
            self.pairs.get_mut(&id)
        }
    }

    #[inline]
    pub fn remove(&mut self, id: Id) -> Option<V> {
        if id.is_entity() {
            self.ids.remove(&id)
        } else {
            self.pairs.remove(&id)
        }
    }
}
