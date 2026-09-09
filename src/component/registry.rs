use std::collections::hash_map::Entry;

use ahash::AHashMap;

use crate::{
    ComponentId, ComponentKey,
    component::{ComponentConfig, ComponentInfo, ComponentRegisterError, Path, id::PREFIX},
    key::component_key_count,
    macros::impl_registry,
    type_meta::HasMeta,
};

pub struct ComponentRegistry {
    infos: Vec<ComponentInfo>,
    keys: Box<[Option<ComponentId>]>,
    id_by_path: AHashMap<Path, ComponentId>,
}

impl_registry!(ComponentRegistry[ComponentId] -> ComponentInfo);

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            infos: Vec::new(),
            keys: vec![None; component_key_count()].into_boxed_slice(),
            id_by_path: AHashMap::new(),
        }
    }

    pub fn register<T: HasMeta>(&mut self, key: ComponentKey<T>) -> Result<ComponentId, ComponentRegisterError> {
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
        let id = ComponentId::new(self.infos.len() as u32).expect("too many components");
        let path = match config.path {
            None => id.to_string().into(),
            Some(p) => {
                if p.starts_with(PREFIX) {
                    return Err(ComponentRegisterError::ReservedPrefix(p));
                }
                match self.id_by_path.entry(p.clone()) {
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

    pub fn find_by_path(&self, path: &str) -> Option<ComponentId> {
        match path.strip_prefix(PREFIX) {
            Some(n) => n
                .parse()
                .ok()
                .and_then(ComponentId::new)
                .filter(|id| id.index() < self.infos.len()),
            None => self.id_by_path.get(path).copied(),
        }
    }

    #[inline(always)]
    pub fn find<T: HasMeta>(&self, key: ComponentKey<T>) -> Option<ComponentId> {
        self.keys[key.slot() as usize]
    }
}
