use std::{collections::hash_map::Entry, rc::Rc};

use ahash::AHashMap;

use crate::{
    TypeMeta,
    relation::index::{RelationIndex, RelationProperties, SymmetryError},
};

pub mod index;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
pub struct RelationId(u32);

impl std::fmt::Display for RelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Relation#{}", self.0)
    }
}

pub struct RelationInfo {
    name: Option<Rc<str>>,
    index: RelationIndex,
    edge_meta: Option<Rc<TypeMeta>>,
}

pub struct RelationRegistry {
    infos: Vec<RelationInfo>,
    names: AHashMap<Rc<str>, RelationId>,
}

impl RelationRegistry {
    pub fn new() -> Self {
        Self { infos: vec![], names: AHashMap::new() }
    }

    pub fn register(
        &mut self,
        name: Option<Rc<str>>,
        props: RelationProperties,
        edge_meta: Option<Rc<TypeMeta>>,
    ) -> Result<RelationId, SymmetryError> {
        let id = RelationId(self.infos.len() as u32);

        let info = RelationInfo {
            name: name.clone(),
            index: RelationIndex::select(props)?,
            edge_meta,
        };

        if let Some(name) = name {
            match self.names.entry(name) {
                Entry::Vacant(e) => e.insert(id),
                Entry::Occupied(e) => panic!("duplicate relation name `{}`", e.key()),
            };
        }

        self.infos.push(info);
        Ok(id)
    }

    pub fn get(&self, id: RelationId) -> &RelationInfo {
        &self.infos[id.0 as usize]
    }

    pub fn get_mut(&mut self, id: RelationId) -> &mut RelationInfo {
        &mut self.infos[id.0 as usize]
    }

    #[inline(always)]
    pub fn index(&self, id: RelationId) -> &RelationIndex {
        &self.infos[id.0 as usize].index
    }

    #[inline(always)]
    pub fn index_mut(&mut self, id: RelationId) -> &mut RelationIndex {
        &mut self.infos[id.0 as usize].index
    }
}
