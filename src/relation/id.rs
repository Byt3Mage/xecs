use crate::{Ecs, key::RelationKey, macros::declare_id, relation::RelationInfo};

/// Reserved: `R#<n>` addresses a relation by id in string queries, and
/// is the display form of an unnamed relation.
pub(super) const PREFIX: &str = "R";

declare_id!(RelationId, PREFIX);

pub trait IntoRelationId: Sized {
    fn id(self, ecs: &Ecs) -> Option<RelationId>;

    fn relation(self, ecs: &Ecs) -> Option<&RelationInfo> {
        self.id(ecs).map(|id| &ecs.relations[id])
    }
}

impl IntoRelationId for &RelationKey {
    #[inline(always)]
    fn id(self, ecs: &Ecs) -> Option<RelationId> {
        ecs.relations.find(self)
    }
}

impl IntoRelationId for &str {
    #[inline(always)]
    fn id(self, ecs: &Ecs) -> Option<RelationId> {
        ecs.relations.find_by_path(self)
    }
}
