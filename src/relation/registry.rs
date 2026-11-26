use crate::{
    RelationId,
    id::Id,
    relation::{RelationInfo, static_relations},
};

/// Resuable buffers for graph searches over one relation.
#[derive(Default)]
pub struct Walk {
    stack: Vec<Id>,
    stamps: Vec<u32>,
    stamp: u32,
}

impl Walk {
    pub(crate) fn begin(&mut self) {
        self.stack.clear();
        self.stamp = self.stamp.checked_add(1).unwrap_or_else(|| {
            self.stamps.fill(0);
            1
        });
    }

    #[inline]
    pub(crate) fn push(&mut self, id: Id) {
        self.stack.push(id)
    }

    #[inline]
    pub(crate) fn pop(&mut self) -> Option<Id> {
        self.stack.pop()
    }

    pub(crate) fn mark(&mut self, id: Id) -> bool {
        let i = id.index as usize;
        if i >= self.stamps.len() {
            self.stamps.resize(i + 1, 0);
        }
        let slot = std::mem::replace(&mut self.stamps[i], self.stamp);
        slot != self.stamp
    }
}

pub struct RelationRegistry {
    pub(crate) infos: Vec<RelationInfo>,
    pub(crate) walk: Walk,
}

impl RelationRegistry {
    pub fn new() -> Self {
        let keys = &static_relations().keys;
        let infos = keys.iter().map(|k| RelationInfo::from_key(k).unwrap()).collect();
        Self { infos, walk: Walk::default() }
    }
}

impl std::ops::Index<RelationId> for RelationRegistry {
    type Output = RelationInfo;

    #[inline]
    fn index(&self, index: RelationId) -> &Self::Output {
        &self.infos[index.index()]
    }
}

impl std::ops::IndexMut<RelationId> for RelationRegistry {
    #[inline]
    fn index_mut(&mut self, index: RelationId) -> &mut Self::Output {
        &mut self.infos[index.index()]
    }
}
