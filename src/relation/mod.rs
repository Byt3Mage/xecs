use std::{collections::hash_map::Entry, sync::Arc};

use ahash::AHashMap;

use crate::{
    Id, RelationKey, Shape, TypeMeta,
    key::relation_key_count,
    relation::{
        id::RelationId,
        storage::{Edges, RelateError, Storage},
    },
    type_meta::HasMeta,
};

pub mod directed;
pub mod hierarchy;
pub mod id;
pub mod storage;
pub mod symmetry;

type Path = Arc<str>;

/// Reserved: `R#<n>` addresses a relation by id in string queries, and
/// is the display form of an unnamed component.
pub const ID_PREFIX: &str = "R#";

#[derive(Debug, thiserror::Error)]
pub enum RelationRegisterError {
    #[error("relation path `{0}` is already registered")]
    DuplicatePath(Path),
    #[error("relation path `{0}` uses the reserved `{ID_PREFIX}` prefix")]
    ReservedPrefix(Path),
}

pub struct RelationInfo {
    path: Arc<str>,
    meta: TypeMeta,
    shape: Shape,
    storage: Storage,
}

impl RelationInfo {
    pub fn shape(&self) -> Shape {
        self.shape
    }

    pub(crate) unsafe fn relate<T>(&mut self, source: Id, target: Id, payload: T) -> Result<(), RelateError> {
        unsafe {
            match &mut self.storage {
                Storage::Directed(s) => s.relate(source, target, payload),
                Storage::Symmetry(s) => s.relate(source, target, payload),
                Storage::Hierarchy(s) => s.relate(source, target, payload),
            }
        }
    }

    pub(crate) fn unrelate(&mut self, source: Id, target: Id) {
        match &mut self.storage {
            Storage::Directed(s) => s.unrelate(source, target),
            Storage::Symmetry(s) => s.unrelate(source, target),
            Storage::Hierarchy(s) => s.unrelate(source, target),
        }
    }

    pub(crate) fn contains(&self, source: Id, target: Id) -> bool {
        match &self.storage {
            Storage::Directed(s) => s.contains(source, target),
            Storage::Symmetry(s) => s.contains(source, target),
            Storage::Hierarchy(s) => s.contains(source, target),
        }
    }

    pub(crate) fn outgoing(&self, source: Id) -> Option<Edges<'_>> {
        match &self.storage {
            Storage::Directed(s) => s.outgoing(source),
            Storage::Symmetry(s) => s.neighbors(source),
            Storage::Hierarchy(s) => s.outgoing(source).map(Edges::One),
        }
    }

    pub(crate) fn incoming(&self, target: Id) -> Option<Edges<'_>> {
        match &self.storage {
            Storage::Directed(s) => s.incoming(target),
            Storage::Symmetry(s) => s.neighbors(target),
            Storage::Hierarchy(s) => s.incoming(target).map(Edges::Children),
        }
    }

    pub(crate) fn has_outgoing(&self, target: Id) -> bool {
        match &self.storage {
            Storage::Directed(s) => s.has_outgoing(target),
            Storage::Symmetry(s) => s.has_edges(target),
            Storage::Hierarchy(s) => s.has_outgoing(target),
        }
    }

    pub(crate) fn has_incoming(&self, target: Id) -> bool {
        match &self.storage {
            Storage::Directed(s) => s.has_incoming(target),
            Storage::Symmetry(s) => s.has_edges(target),
            Storage::Hierarchy(s) => s.has_incoming(target),
        }
    }

    pub(crate) fn purge(&mut self, id: Id) {
        match &mut self.storage {
            Storage::Directed(d) => d.purge(id),
            Storage::Symmetry(s) => s.purge(id),
            Storage::Hierarchy(h) => h.purge(id),
        }
    }
}
pub struct RelationRegistry {
    infos: Vec<RelationInfo>,
    paths: AHashMap<Path, RelationId>,
    keys: Box<[Option<RelationId>]>,
}

impl RelationRegistry {
    pub fn new() -> Self {
        Self {
            infos: vec![],
            paths: AHashMap::new(),
            keys: vec![None; relation_key_count()].into_boxed_slice(),
        }
    }

    pub fn register<T: HasMeta>(&mut self, key: &RelationKey<T>) -> Result<RelationId, RelationRegisterError> {
        let slot = key.slot() as usize;
        Ok(match self.keys[slot] {
            Some(id) => id,
            None => {
                let id = self.new_relation(Some(key.path().into()), key.topo(), *T::META)?;
                self.keys[slot] = Some(id);
                id
            }
        })
    }

    pub fn new_relation(
        &mut self,
        path: Option<Path>,
        shape: Shape,
        meta: TypeMeta,
    ) -> Result<RelationId, RelationRegisterError> {
        debug_assert!(self.infos.len() <= (u32::MAX as usize), "too many relations");
        let id = RelationId::from_raw(self.infos.len() as u32);
        let path = match path {
            None => id.to_string().into(),
            Some(p) => {
                if p.starts_with(ID_PREFIX) {
                    return Err(RelationRegisterError::ReservedPrefix(p));
                }
                match self.paths.entry(p.clone()) {
                    Entry::Vacant(e) => e.insert(id),
                    Entry::Occupied(_) => return Err(RelationRegisterError::DuplicatePath(p)),
                };
                p
            }
        };
        let storage = Storage::select(shape, &meta);
        self.infos.push(RelationInfo { path, shape, storage, meta });
        Ok(id)
    }

    pub fn find_by_name(&self, name: &str) -> Option<RelationId> {
        match name.strip_prefix(ID_PREFIX) {
            Some(n) => {
                let id = n.parse().ok()?;
                ((id as usize) < self.infos.len()).then(|| RelationId::from_raw(id))
            }
            None => self.paths.get(name).copied(),
        }
    }

    #[inline(always)]
    pub fn find<T: HasMeta>(&self, key: &RelationKey<T>) -> Option<RelationId> {
        self.keys[key.slot() as usize]
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
