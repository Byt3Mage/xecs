use std::{
    collections::hash_map::Entry,
    ops::{Index, IndexMut},
    rc::Rc,
};

use ahash::AHashMap;

use super::{ComponentConfig, ComponentId, ComponentInfo, traits::TraitInfo};

pub struct ComponentRegistry {
    infos: Vec<ComponentInfo>,
    names: AHashMap<Rc<str>, ComponentId>,
    traits: Vec<TraitInfo>,
    static_ids: Vec<Option<ComponentId>>,
}
impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            infos: Vec::new(),
            names: AHashMap::new(),
            traits: Vec::new(),
            static_ids: Vec::new(),
        }
    }

    pub fn register(&mut self, config: ComponentConfig) -> ComponentId {
        let id = ComponentId(self.infos.len() as u32);
        let name = config.name;

        if let Some(name) = name.clone() {
            match self.names.entry(name) {
                Entry::Vacant(e) => e.insert(id),
                Entry::Occupied(e) => panic!("duplicate component name `{}`", e.key()),
            };
        }

        let meta = Rc::new(config.meta);
        self.infos.push(ComponentInfo { name, meta, tables: vec![] });

        id
    }

    pub fn get(&self, id: ComponentId) -> &ComponentInfo {
        &self.infos[id.0 as usize]
    }

    pub fn get_mut(&mut self, id: ComponentId) -> &mut ComponentInfo {
        &mut self.infos[id.0 as usize]
    }
}

impl IndexMut<ComponentId> for ComponentRegistry {
    #[inline]
    fn index_mut(&mut self, index: ComponentId) -> &mut Self::Output {
        &mut self.infos[index.0 as usize]
    }
}

impl Index<ComponentId> for ComponentRegistry {
    type Output = ComponentInfo;

    #[inline]
    fn index(&self, index: ComponentId) -> &Self::Output {
        &self.infos[index.0 as usize]
    }
}
