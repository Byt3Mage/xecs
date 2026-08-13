use std::{
    collections::hash_map::Entry,
    ops::{Index, IndexMut},
};

use ahash::AHashMap;

use crate::{
    ComponentId, ComponentKey,
    component::{ComponentConfig, ComponentInfo, ComponentRegisterError, ID_PREFIX, Path},
    key::component_key_count,
    type_meta::HasMeta,
};

pub struct ComponentRegistry {
    infos: Vec<ComponentInfo>,
    paths: AHashMap<Path, ComponentId>,
    keys: Box<[Option<ComponentId>]>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            infos: Vec::new(),
            paths: AHashMap::new(),
            keys: vec![None; component_key_count()].into_boxed_slice(),
        }
    }

    pub fn register<T: HasMeta>(&mut self, key: &ComponentKey<T>) -> Result<ComponentId, ComponentRegisterError> {
        let slot = key.slot() as usize;
        Ok(match self.keys[slot] {
            Some(id) => id,
            None => {
                let config = ComponentConfig { path: Some(key.path().into()), meta: *T::META };
                let id = self.new_component(config)?;
                self.keys[slot] = Some(id);
                id
            }
        })
    }

    pub fn new_component(&mut self, config: ComponentConfig) -> Result<ComponentId, ComponentRegisterError> {
        debug_assert!(self.infos.len() <= (u32::MAX as usize), "too many components");
        let id = ComponentId::from_raw(self.infos.len() as u32);
        let path = match config.path {
            None => id.to_string().into(),
            Some(p) => {
                if p.starts_with(ID_PREFIX) {
                    return Err(ComponentRegisterError::ReservedPrefix(p));
                }
                match self.paths.entry(p.clone()) {
                    Entry::Vacant(e) => e.insert(id),
                    Entry::Occupied(_) => return Err(ComponentRegisterError::DuplicatePath(p)),
                };
                p
            }
        };

        self.infos
            .push(ComponentInfo { path, meta: config.meta, tables: vec![] });
        Ok(id)
    }

    pub fn get(&self, id: ComponentId) -> &ComponentInfo {
        &self.infos[id.index()]
    }

    pub fn find_by_name(&self, name: &str) -> Option<ComponentId> {
        match name.strip_prefix(ID_PREFIX) {
            Some(n) => {
                let id = n.parse().ok()?;
                ((id as usize) < self.infos.len()).then(|| ComponentId::from_raw(id))
            }
            None => self.paths.get(name).copied(),
        }
    }

    #[inline(always)]
    pub fn find<T: HasMeta>(&self, key: &ComponentKey<T>) -> Option<ComponentId> {
        self.keys[key.slot() as usize]
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
