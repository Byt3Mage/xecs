use crate::{
    ComponentId,
    component::{ComponentDesc, ComponentInfo, static_components},
};

pub struct ComponentRegistry {
    infos: Vec<ComponentInfo>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        let keys = static_components();
        let infos = keys.iter().map(|k| ComponentInfo::from_meta(k.meta())).collect();
        Self { infos }
    }

    pub fn new_component(&mut self, desc: ComponentDesc) -> ComponentId {
        let id = ComponentId::from_raw(self.infos.len() as u32).expect("too many components");
        self.infos.push(ComponentInfo::new(desc.layout));
        id
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
