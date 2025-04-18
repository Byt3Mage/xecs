use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

use crate::id::HI_COMPONENT_ID;

pub type Entity = u64;

/// Tag used to indicate an entity is a module.
pub const ECS_MODULE: Entity = HI_COMPONENT_ID + 4;
/// Tag used to indicate an entity is private to a module.
pub const ECS_PRIVATE: Entity = HI_COMPONENT_ID + 5;
/// Tag used to indicate an entity is a prefab.
pub const ECS_PREFAB: Entity = HI_COMPONENT_ID + 6;
/// When added to an entity, the tag is skipped by queries,
/// unless DISABLED is explicitly queried for.
pub const ECS_DISABLED: Entity = HI_COMPONENT_ID + 7;
/// Tag used for entities that should never be returned by queries.
/// Used for entities that have special meaning to the query engine.
pub const ECS_NOT_QUERYABLE: Entity = HI_COMPONENT_ID + 8;

/// Used for slots in prefabs.
pub const ECS_SLOT_OF: Entity = HI_COMPONENT_ID + 9;
/// Used to track entities used with id flags.
pub const ECS_FLAG: Entity = HI_COMPONENT_ID + 10;

/* Marker entities for query encoding */
pub const ECS_WILDCARD: Entity = HI_COMPONENT_ID + 11;
pub const ECS_ANY: Entity = HI_COMPONENT_ID + 12;
pub const ECS_THIS: Entity = HI_COMPONENT_ID + 13;
pub const ECS_VARIABLE: Entity = HI_COMPONENT_ID + 14;

/* Builtin relationships */
pub const ECS_CHILD_OF: Entity = HI_COMPONENT_ID + 34;
pub const ECS_IS_A: Entity = HI_COMPONENT_ID + 35;
pub const ECS_DEPENDS_ON: Entity = HI_COMPONENT_ID + 36;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityFlags(u64);

impl EntityFlags {
    pub const IS_ID: Self = Self(1 << 0);
    pub const IS_TARGET: Self = Self(1 << 1);
    pub const IS_TRAVERSABLE: Self = Self(1 << 2);
    pub const HAS_SPARSE: Self = Self(1 << 3);

    #[inline]
    /// Returns an empty set of flags.
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub const fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    #[inline]
    pub const fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl Default for EntityFlags {
    fn default() -> Self {
        Self::empty()
    }
}

impl BitOr for EntityFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for EntityFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for EntityFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for EntityFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitXor for EntityFlags {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for EntityFlags {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for EntityFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}