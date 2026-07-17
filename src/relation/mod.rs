use std::{collections::hash_map::Entry, rc::Rc};

use ahash::AHashMap;

use crate::{TypeMeta, relation::index::RelationIndex};

pub mod index;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
pub struct RelationId(u32);

impl std::fmt::Display for RelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Relation#{}", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RelationProps {
    pub unique_source: bool,
    pub unique_target: bool,
    pub acyclic: bool,
    pub symmetric: bool,
    pub indexed_reverse: bool,
}

pub struct RelationInfo {
    pub name: Option<Rc<str>>,
    pub props: RelationProps,
    pub index: RelationIndex,
    pub edge_meta: Option<Rc<TypeMeta>>,
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
        props: RelationProps,
        edge_meta: Option<Rc<TypeMeta>>,
    ) -> RelationId {
        let id = RelationId(self.infos.len() as u32);

        if let Some(name) = name.clone() {
            match self.names.entry(name) {
                Entry::Vacant(e) => e.insert(id),
                Entry::Occupied(e) => panic!("duplicate relation name `{}`", e.key()),
            };
        }

        let index = RelationIndex::select(&props, edge_meta.clone());
        self.infos.push(RelationInfo { name, props, index, edge_meta });

        id
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
}
