use std::collections::hash_map::Entry;

use ahash::AHashMap;

use crate::{
    ComponentId,
    component::{ComponentConfig, ComponentInfo, ComponentRegisterError, PREFIX, Path, static_components},
};

pub struct ComponentRegistry {
    infos: Vec<ComponentInfo>,
    path_to_id: AHashMap<Path, ComponentId>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        let keys = static_components();
        let mut registry = Self {
            infos: Vec::with_capacity(keys.len()),
            path_to_id: AHashMap::with_capacity(keys.len()),
        };

        for key in keys {
            let path = Path::Static(key.path());
            registry.infos.push(ComponentInfo::new(path.clone(), *key.meta()));
            registry.path_to_id.insert(path, key.id());
        }

        registry
    }

    pub fn new_component(&mut self, config: ComponentConfig) -> Result<ComponentId, ComponentRegisterError> {
        let id = ComponentId::new(self.infos.len() as u32).expect("too many components");
        let path = match config.path {
            None => id.to_string().into(),
            Some(p) => {
                if p.starts_with(PREFIX) {
                    return Err(ComponentRegisterError::ReservedPrefix(p));
                }
                match self.path_to_id.entry(p.clone()) {
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

    pub fn find_by_path(&self, path: &str) -> Option<ComponentId> {
        match path.strip_prefix(PREFIX) {
            Some(n) => n
                .parse()
                .ok()
                .and_then(ComponentId::new)
                .filter(|id| id.index() < self.infos.len()),
            None => self.path_to_id.get(path).copied(),
        }
    }
}

impl std::ops::IndexMut<ComponentId> for ComponentRegistry {
    #[inline]
    fn index_mut(&mut self, index: ComponentId) -> &mut Self::Output {
        &mut self.infos[index.index()]
    }
}

impl std::ops::Index<ComponentId> for ComponentRegistry {
    type Output = ComponentInfo;

    #[inline]
    fn index(&self, index: ComponentId) -> &Self::Output {
        &self.infos[index.index()]
    }
}
