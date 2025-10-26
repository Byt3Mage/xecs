pub(crate) mod manager;

use crate::{
    data_structures::{SparseIndex, SparseSet},
    world::World,
};
use std::{collections::HashMap, fmt::Display, ops::Deref, rc::Rc};

/// FFI compatible representation of an id.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(u64);

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let idx = self.index();
        let ver = self.generation();
        write!(f, "Entity({idx}, v{ver})")
    }
}

impl Id {
    // Id Flags
    pub const PAIR_FLAG: u64 = 1u64 << 63;
    pub const MAX_TGT_ID: u64 = 0x7FFF_FFFF;

    /// Built-in entities
    pub const NULL: Id = Id(u64::MAX);
    pub const WILDCARD: Id = Id(1);

    /// Creates a new `Entity` from raw bits.
    #[inline(always)]
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

    /// Increments the generation counter.
    pub(crate) const fn inc_gen(&self) -> Self {
        Self((((self.0 >> 32) + 1) << 32) | (self.index() as u64))
    }

    pub const fn is_wildcard(&self) -> bool {
        self.0 == Self::WILDCARD.0
    }

    pub const fn from_parts(index: u32, generation: u32) -> Self {
        Self(((generation as u64) << 32) | index as u64)
    }
}

#[inline(always)]
pub(crate) const fn pair(rel: Id, tgt: Id) -> Id {
    Id((tgt.index() as u64) | ((rel.index() as u64) << 32) | Id::PAIR_FLAG)
}

impl SparseIndex for Id {
    fn to_sparse_index(&self) -> usize {
        self.index() as usize
    }
}

/// This trait should never be implemented by users.
/// There is no safe way to implement this trait.
pub unsafe trait IntoId {
    fn validate(&self, world: &World) -> bool;
    fn into_id(self) -> Id;
}

unsafe impl IntoId for Id {
    fn validate(&self, world: &World) -> bool {
        world.is_alive(*self)
    }

    fn into_id(self) -> Id {
        self
    }
}

unsafe impl IntoId for (Id, Id) {
    fn validate(&self, world: &World) -> bool {
        let (rel, tgt) = *self;
        world.is_alive(rel) && world.is_alive(tgt)
    }

    fn into_id(self) -> Id {
        pair(self.0, self.1)
    }
}

/// Sorted list of ids in a [Table](crate::storage::table::Table)
#[derive(Hash, PartialEq, Eq)]
pub struct Signature(Rc<[Id]>);

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

impl From<Vec<Id>> for Signature {
    fn from(mut value: Vec<Id>) -> Self {
        Self({
            value.sort();
            value.dedup();
            value.into()
        })
    }
}

impl<const N: usize> From<[Id; N]> for Signature {
    fn from(value: [Id; N]) -> Self {
        Self({
            let mut vec = Vec::from(value);
            vec.sort();
            vec.dedup();
            vec.into()
        })
    }
}

impl Deref for Signature {
    type Target = [Id];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Signature {
    #[inline]
    pub fn ids(&self) -> &[Id] {
        &self.0
    }

    #[inline]
    pub fn has_id(&self, id: Id) -> bool {
        self.binary_search(&id).is_ok()
    }

    /// Creates a new sorted list from [Self](IdList) and `with`
    ///
    /// Returns `None` if self already contains `with`.
    pub fn try_extend(&self, with: Id) -> Option<Self> {
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
    pub fn try_shrink(&self, from: Id) -> Option<Self> {
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Relation {
    rel: Id,
    tgt: Id,
}

pub trait Key {
    fn map_get<'a, V>(&self, map: &'a KeyMap<V>) -> Option<&'a V>;
    fn map_get_mut<'a, V>(&self, map: &'a mut KeyMap<V>) -> Option<&'a mut V>;
}

impl Key for Id {
    fn map_get<'a, V>(&self, map: &'a KeyMap<V>) -> Option<&'a V> {
        map.ids.get(&self)
    }

    fn map_get_mut<'a, V>(&self, map: &'a mut KeyMap<V>) -> Option<&'a mut V> {
        map.ids.get_mut(&self)
    }
}
impl Key for Relation {
    fn map_get<'a, V>(&self, map: &'a KeyMap<V>) -> Option<&'a V> {
        map.rels.get(self)
    }

    fn map_get_mut<'a, V>(&self, map: &'a mut KeyMap<V>) -> Option<&'a mut V> {
        map.rels.get_mut(&self)
    }
}

pub struct KeyMap<V> {
    ids: SparseSet<Id, V>,
    rels: HashMap<Relation, V>,
}

impl<V> KeyMap<V> {
    pub fn new() -> Self {
        Self {
            ids: SparseSet::new(),
            rels: HashMap::new(),
        }
    }

    #[inline(always)]
    pub fn get<'a, K: Key>(&'a self, key: &K) -> Option<&'a V> {
        key.map_get(self)
    }

    #[inline(always)]
    pub fn get_mut<'a, K: Key>(&'a mut self, key: &K) -> Option<&'a mut V> {
        key.map_get_mut(self)
    }
}
