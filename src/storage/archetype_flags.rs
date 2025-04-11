use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

/// Represents a set of flags for archetypes, used to define various properties
/// and behaviors of archetypes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchetypeFlags(u64);

impl ArchetypeFlags {
    /// Indicates that the archetype has built-in components.
    pub const HAS_BUILTINS: Self = Self(1 << 1);

    /// Indicates that the archetype stores prefabs.
    pub const IS_PREFAB: Self = Self(1 << 2);

    /// Indicates that the archetype has an `IsA` relationship.
    pub const HAS_IS_A: Self = Self(1 << 3);

    /// Indicates that the archetype has a `ChildOf` relationship.
    pub const HAS_CHILD_OF: Self = Self(1 << 4);

    /// Indicates that the archetype has components for `(Identifier, Name)`.
    pub const HAS_NAME: Self = Self(1 << 5);

    /// Indicates that the archetype has pairs.
    pub const HAS_PAIRS: Self = Self(1 << 6);

    /// Indicates that the archetype has module data.
    pub const HAS_MODULE: Self = Self(1 << 7);

    /// Indicates that the archetype has the `EcsDisabled` component.
    pub const IS_DISABLED: Self = Self(1 << 8);

    /// Indicates that the archetype should never be returned by queries.
    pub const NOT_QUERYABLE: Self = Self(1 << 9);

    /// Indicates that the archetype has constructors.
    pub const HAS_CTORS: Self = Self(1 << 10);

    /// Indicates that the archetype has destructors.
    pub const HAS_DTORS: Self = Self(1 << 11);

    /// Indicates that the archetype supports copy semantics.
    pub const HAS_COPY: Self = Self(1 << 12);

    /// Indicates that the archetype supports move semantics.
    pub const HAS_MOVE: Self = Self(1 << 13);

    /// Indicates that the archetype supports toggling.
    pub const HAS_TOGGLE: Self = Self(1 << 14);

    /// Indicates that the archetype has overrides.
    pub const HAS_OVERRIDES: Self = Self(1 << 15);

    pub const HAS_ON_ADD: Self = Self(1 << 16);
    pub const HAS_ON_REMOVE: Self = Self(1 << 17);
    pub const HAS_ON_SET: Self = Self(1 << 18);
    pub const HAS_ON_TABLE_FILL: Self = Self(1 << 19);
    pub const HAS_ON_TABLE_EMPTY: Self = Self(1 << 20);
    pub const HAS_ON_TABLE_CREATE: Self = Self(1 << 21);
    pub const HAS_ON_TABLE_DELETE: Self = Self(1 << 22);
    pub const HAS_SPARSE: Self = Self(1 << 23);
    pub const HAS_UNION: Self = Self(1 << 24);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    pub fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl BitOr for ArchetypeFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ArchetypeFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ArchetypeFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for ArchetypeFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitXor for ArchetypeFlags {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for ArchetypeFlags {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for ArchetypeFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}
