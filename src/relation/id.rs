use crate::{Ecs, key::RelationKey, relation::RelationInfo, type_meta::HasMeta};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
pub struct RelationId(u32);

impl RelationId {
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

impl std::fmt::Display for RelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "R#{}", self.0)
    }
}

pub trait IntoRelationId: Sized {
    fn into_id(self, ecs: &Ecs) -> Option<RelationId>;

    fn component(self, ecs: &Ecs) -> Option<&RelationInfo> {
        self.into_id(ecs).map(|id| &ecs.relations[id])
    }
}

impl<T: HasMeta> IntoRelationId for &RelationKey<T> {
    #[inline(always)]
    fn into_id(self, ecs: &Ecs) -> Option<RelationId> {
        ecs.relations.find(self)
    }
}

impl IntoRelationId for &str {
    #[inline(always)]
    fn into_id(self, ecs: &Ecs) -> Option<RelationId> {
        ecs.relations.find_by_name(self)
    }
}
