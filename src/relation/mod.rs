use std::{collections::hash_map::Entry, sync::Arc};

use ahash::AHashMap;

use crate::{
    Ecs, Id, RelationKey, Shape, TypeMeta,
    key::{UnregisteredKey, relation_key_count, unregistered},
    macros::impl_registry,
    relation::{
        id::{PREFIX, RelationId},
        storage::{Edges, RelateError, Storage},
    },
    type_meta::HasMeta,
};

pub mod directed;
mod edges;
pub mod hierarchy;
pub mod id;
pub mod storage;
pub mod symmetry;

type Path = Arc<str>;

#[derive(Debug, thiserror::Error)]
pub enum RelationRegisterError {
    #[error("relation path `{0}` is already registered")]
    DuplicatePath(Path),
    #[error("relation path `{}` uses the reserved `{}` prefix", .0, PREFIX)]
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
                Storage::Hierarchy(s) => s.relate(source, target),
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
    keys: Box<[Option<RelationId>]>,
    id_by_path: AHashMap<Path, RelationId>,
}

impl_registry!(RelationRegistry[RelationId] -> RelationInfo);

impl RelationRegistry {
    pub fn new() -> Self {
        Self {
            infos: vec![],
            id_by_path: AHashMap::new(),
            keys: vec![None; relation_key_count()].into_boxed_slice(),
        }
    }

    pub fn register<T: HasMeta>(&mut self, key: &RelationKey) -> Result<RelationId, RelationRegisterError> {
        let slot = key.slot() as usize;
        Ok(match self.keys[slot] {
            Some(id) => id,
            None => {
                let id = self.new_relation(Some(key.path().into()), key.shape(), *T::META)?;
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
        let id = RelationId::new(self.infos.len() as u32).expect("too many relations");
        let path = match path {
            None => id.to_string().into(),
            Some(p) => {
                if p.starts_with(PREFIX) {
                    return Err(RelationRegisterError::ReservedPrefix(p));
                }
                match self.id_by_path.entry(p.clone()) {
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

    pub fn find_by_path(&self, path: &str) -> Option<RelationId> {
        match path.strip_prefix(PREFIX) {
            Some(n) => n
                .parse()
                .ok()
                .and_then(RelationId::new)
                .filter(|id| id.index() < self.infos.len()),
            None => self.id_by_path.get(path).copied(),
        }
    }

    #[inline(always)]
    pub fn find(&self, key: &RelationKey) -> Option<RelationId> {
        self.keys[key.slot() as usize]
    }
}

/// Resolve `key` to the id it registered under.
///
/// This is the sole bridge between a typed key and an untyped column,
/// and the sole justification for the `*_raw` calls below: the slot →
/// id mapping is written once, at registration, from `T::META` of this
/// same key type. A `ComponentId` obtained any other way (`new_component`,
/// `find_by_name`) cannot reach these functions, because they take a key.
#[inline(always)]
pub fn resolve(ecs: &Ecs, key: &RelationKey) -> Result<RelationId, UnregisteredKey> {
    ecs.relations.find(key).ok_or_else(|| unregistered(key.untyped()))
}
