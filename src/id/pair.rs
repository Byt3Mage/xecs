use std::fmt::Display;

use crate::id::{Entity, Id, IdMap};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Pair {
    pub rel: Entity,
    pub tgt: Entity,
}

impl Id for Pair {
    #[inline(always)]
    fn map_insert<V>(self, map: &mut IdMap<V>, value: V) -> Option<V> {
        map.pairs.insert(self, value)
    }

    #[inline(always)]
    fn map_get<'a, V>(&self, map: &'a IdMap<V>) -> Option<&'a V> {
        map.pairs.get(self)
    }

    #[inline(always)]
    fn map_get_mut<'a, V>(&self, map: &'a mut IdMap<V>) -> Option<&'a mut V> {
        map.pairs.get_mut(self)
    }

    #[inline(always)]
    fn map_contains_key<V>(&self, map: &IdMap<V>) -> bool {
        map.pairs.contains_key(self)
    }
}

impl Display for Pair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.rel, self.tgt)
    }
}
