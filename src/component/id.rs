use crate::{Ecs, component::ComponentInfo, key::ComponentKey, macros::declare_id, type_meta::HasMeta};

/// Reserved: `C#<n>` addresses a component by id in string queries, and
/// is the display form of an unnamed component.
pub(super) const PREFIX: &str = "C#";

declare_id!(ComponentId, PREFIX);

pub trait IntoComponentId: Sized {
    fn id(self, ecs: &Ecs) -> Option<ComponentId>;

    fn component(self, ecs: &Ecs) -> Option<&ComponentInfo> {
        self.id(ecs).map(|id| &ecs.components[id])
    }
}

impl<T: HasMeta> IntoComponentId for ComponentKey<T> {
    #[inline(always)]
    fn id(self, ecs: &Ecs) -> Option<ComponentId> {
        ecs.components.find(self)
    }
}

impl IntoComponentId for &str {
    #[inline(always)]
    fn id(self, ecs: &Ecs) -> Option<ComponentId> {
        ecs.components.find_by_path(self)
    }
}
