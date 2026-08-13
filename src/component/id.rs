use crate::{Ecs, component::ComponentInfo, key::ComponentKey, type_meta::HasMeta};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(transparent)]
pub struct ComponentId(u32);

impl ComponentId {
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "C#{}", self.0)
    }
}

pub trait IntoComponentId: Sized {
    fn into_id(self, ecs: &Ecs) -> Option<ComponentId>;

    fn component(self, ecs: &Ecs) -> Option<&ComponentInfo> {
        self.into_id(ecs).map(|id| &ecs.components[id])
    }
}

impl<T: HasMeta> IntoComponentId for &ComponentKey<T> {
    #[inline(always)]
    fn into_id(self, ecs: &Ecs) -> Option<ComponentId> {
        ecs.components.find(self)
    }
}

impl IntoComponentId for &str {
    #[inline(always)]
    fn into_id(self, ecs: &Ecs) -> Option<ComponentId> {
        ecs.components.find_by_name(self)
    }
}
