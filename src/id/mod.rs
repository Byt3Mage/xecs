pub(crate) mod entity_manager;
pub mod pair;

use crate::data_structures::{SparseIndex, SparseSet};
use ahash::AHashMap;
use pair::Pair;
use std::{fmt::Display, ops::Deref, rc::Rc};

pub trait Id: Clone + Display {
    fn map_insert<V>(self, map: &mut IdMap<V>, value: V) -> Option<V>;
    fn map_get<'a, V>(&self, map: &'a IdMap<V>) -> Option<&'a V>;
    fn map_get_mut<'a, V>(&self, map: &'a mut IdMap<V>) -> Option<&'a mut V>;
    fn map_contains_key<V>(&self, map: &IdMap<V>) -> bool;
}

/// FFI compatible representation of an id.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Entity(u64);

impl SparseIndex for Entity {
    #[inline(always)]
    fn idx(&self) -> usize {
        self.idx() as usize
    }
}

impl Id for Entity {
    #[inline(always)]
    fn map_insert<V>(self, map: &mut IdMap<V>, value: V) -> Option<V> {
        match map.id_lo.get_mut(self.as_usize()) {
            Some(v) => v.replace(value),
            None => map.id_hi.insert(self, value),
        }
    }

    #[inline(always)]
    fn map_get<'a, V>(&self, map: &'a IdMap<V>) -> Option<&'a V> {
        match map.id_lo.get(self.as_usize()) {
            Some(v) => v.as_ref(),
            None => map.id_hi.get(self),
        }
    }

    #[inline(always)]
    fn map_get_mut<'a, V>(&self, map: &'a mut IdMap<V>) -> Option<&'a mut V> {
        match map.id_lo.get_mut(self.as_usize()) {
            Some(v) => v.as_mut(),
            None => map.id_hi.get_mut(self),
        }
    }

    #[inline(always)]
    fn map_contains_key<V>(&self, map: &IdMap<V>) -> bool {
        match map.id_lo.get(self.as_usize()) {
            Some(v) => v.is_some(),
            None => map.id_hi.contains_key(self),
        }
    }
}

impl Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entity({}, v{})", self.idx(), self.ver())
    }
}

impl Entity {
    /// Built-in entities
    pub const NULL: Entity = Entity(u64::MAX);
    pub const WILDCARD: Entity = Entity(0);

    /// Creates a new `Entity` from raw bits.
    #[inline(always)]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Converts the `Entity` back to raw bits.
    #[inline(always)]
    pub const fn to_raw(&self) -> u64 {
        self.0
    }

    #[inline(always)]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// Returns the ID (lower 32 bits).
    #[inline(always)]
    pub const fn idx(&self) -> u32 {
        self.0 as u32
    }

    /// Returns the version (higher 32 bits).
    #[inline(always)]
    pub const fn ver(&self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Increments the version counter.
    pub(crate) const fn inc_ver(&self) -> Self {
        Self((((self.0 >> 32) + 1) << 32) | (self.idx() as u64))
    }

    // const version of equality comparison
    #[inline(always)]
    pub const fn equals(self, other: Self) -> bool {
        self.0 == other.0
    }

    pub const fn from_parts(idx: u32, ver: u32) -> Self {
        Self(((ver as u64) << 32) | idx as u64)
    }
}

/// Sorted list of ids in a [Table](crate::storage::table::Table)
#[derive(Hash, PartialEq, Eq)]
pub struct Signature(Rc<[Entity]>);

impl Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Clone for Signature {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl From<Vec<Entity>> for Signature {
    fn from(mut value: Vec<Entity>) -> Self {
        Self({
            value.sort();
            value.dedup();
            value.into()
        })
    }
}

impl<const N: usize> From<[Entity; N]> for Signature {
    fn from(value: [Entity; N]) -> Self {
        Self({
            let mut vec = Vec::from(value);
            vec.sort();
            vec.dedup();
            vec.into()
        })
    }
}

impl Deref for Signature {
    type Target = [Entity];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Signature {
    #[inline]
    pub fn ids(&self) -> &[Entity] {
        &self.0
    }

    #[inline]
    pub fn has_id(&self, id: Entity) -> bool {
        self.binary_search(&id).is_ok()
    }

    /// Creates a new sorted list from [Self](IdList) and `with`
    ///
    /// Returns `None` if self already contains `with`.
    pub fn try_extend(&self, with: Entity) -> Option<Self> {
        match self.binary_search(&with) {
            Ok(_) => None,
            Err(pos) => Some({
                let mut new_list = Vec::with_capacity(pos);
                new_list.extend_from_slice(&self[..pos]);
                new_list.push(with);
                new_list.extend_from_slice(&self[pos..]);
                new_list.into()
            }),
        }
    }

    /// Creates a new sorted list from [Self](IdList) without `from`.
    ///
    /// Returns `None` if self doesn't contain `from`.
    pub fn try_shrink(&self, from: Entity) -> Option<Self> {
        match self.binary_search(&from) {
            Ok(pos) => Some({
                let mut new_list = Vec::from(self.as_ref());
                new_list.remove(pos);
                new_list.into()
            }),
            Err(_) => None,
        }
    }
}

pub struct IdMap<V> {
    id_lo: Box<[Option<V>]>,
    id_hi: SparseSet<Entity, V>,
    pairs: AHashMap<Pair, V>,
}

impl<V> IdMap<V> {
    pub fn new(max_low_id: usize) -> Self {
        Self {
            id_lo: std::iter::repeat_with(|| None).take(max_low_id).collect(),
            id_hi: SparseSet::new(),
            pairs: AHashMap::new(),
        }
    }
}
