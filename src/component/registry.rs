use std::{
    ops::{Index, IndexMut},
    sync::Arc,
};

use ahash::AHashMap;

use crate::{
    StaticId, UntypedStaticId,
    component::{ComponentConfig, id},
    type_meta::HasMeta,
};

use super::{ComponentId, ComponentInfo};

#[derive(Debug, thiserror::Error)]
#[error("unregistered static component: {0}")]
pub struct Unregistered(&'static UntypedStaticId);

impl Unregistered {
    pub fn path(&self) -> &'static str {
        self.0.path()
    }
}

pub struct ComponentRegistry {
    infos: Vec<ComponentInfo>,
    names: AHashMap<Arc<str>, ComponentId>,
    statics: Box<[Option<ComponentId>]>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            infos: Vec::new(),
            names: AHashMap::new(),
            statics: vec![None; id::static_id_count()].into_boxed_slice(),
        }
    }

    pub fn register<T: HasMeta>(&mut self, component: &StaticId<T>) -> ComponentId {
        let slot = component.slot() as usize;
        match self.statics[slot] {
            Some(id) => id,
            None => {
                let id = self.new_component(ComponentConfig {
                    path: Some(component.path().into()),
                    meta: *component.meta(),
                });
                self.statics[slot] = Some(id);
                id
            }
        }
    }

    pub fn new_component(&mut self, ComponentConfig { path, meta }: ComponentConfig) -> ComponentId {
        debug_assert!(self.infos.len() <= (u32::MAX as usize), "too many components");
        let id = ComponentId::from_raw(self.infos.len() as u32);
        let path = path.map(Into::into);
        path.clone().map(|p| self.names.entry(p).or_insert(id));
        self.infos.push(ComponentInfo { path, meta, tables: vec![] });
        id
    }

    pub fn get(&self, id: ComponentId) -> &ComponentInfo {
        &self.infos[id.index()]
    }

    pub fn get_mut(&mut self, id: ComponentId) -> &mut ComponentInfo {
        &mut self.infos[id.index()]
    }

    pub fn find_by_name(&self, name: &str) -> Option<ComponentId> {
        self.names.get(name).copied()
    }

    pub fn find<T: HasMeta>(&self, id: &'static StaticId<T>) -> Result<ComponentId, Unregistered> {
        self.statics[id.slot() as usize].ok_or_else(|| Unregistered(id.untyped()))
    }
}

impl IndexMut<ComponentId> for ComponentRegistry {
    #[inline]
    fn index_mut(&mut self, index: ComponentId) -> &mut Self::Output {
        &mut self.infos[index.index()]
    }
}

impl Index<ComponentId> for ComponentRegistry {
    type Output = ComponentInfo;

    #[inline]
    fn index(&self, index: ComponentId) -> &Self::Output {
        &self.infos[index.index()]
    }
}
