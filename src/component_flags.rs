use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentFlags(u64);

impl ComponentFlags {
    // OnDelete behavior flags
    pub const ON_DELETE_REMOVE: Self = Self(1 << 0);
    pub const ON_DELETE_DELETE: Self = Self(1 << 1);
    pub const ON_DELETE_PANIC: Self = Self(1 << 2);
    pub const ON_DELETE_MASK: Self = Self(Self::ON_DELETE_REMOVE.0 | Self::ON_DELETE_DELETE.0 | Self::ON_DELETE_PANIC.0);

    // OnDeleteObject behavior flags
    pub const ON_DELETE_OBJECT_REMOVE: Self = Self(1 << 3);
    pub const ON_DELETE_OBJECT_DELETE: Self = Self(1 << 4);
    pub const ON_DELETE_OBJECT_PANIC: Self = Self(1 << 5);
    pub const ON_DELETE_OBJECT_MASK: Self = Self(
        Self::ON_DELETE_OBJECT_REMOVE.0 | Self::ON_DELETE_OBJECT_DELETE.0 | Self::ON_DELETE_OBJECT_PANIC.0
    );

    // OnInstantiate behavior flags
    pub const ON_INSTANTIATE_OVERRIDE: Self = Self(1 << 6);
    pub const ON_INSTANTIATE_INHERIT: Self = Self(1 << 7);
    pub const ON_INSTANTIATE_DONT_INHERIT: Self = Self(1 << 8);
    pub const ON_INSTANTIATE_MASK: Self = Self(
        Self::ON_INSTANTIATE_OVERRIDE.0 | Self::ON_INSTANTIATE_INHERIT.0 | Self::ON_INSTANTIATE_DONT_INHERIT.0
    );

    // Miscellaneous ID flags
    pub const EXCLUSIVE: Self = Self(1 << 9);
    pub const TRAVERSABLE: Self = Self(1 << 10);
    pub const TAG: Self = Self(1 << 11);
    pub const WITH: Self = Self(1 << 12);
    pub const CAN_TOGGLE: Self = Self(1 << 13);
    pub const IS_TRANSITIVE: Self = Self(1 << 14);
    pub const IS_INHERITABLE: Self = Self(1 << 15);

    // Event flags
    pub const HAS_ON_ADD: Self = Self(1 << 16); // Same values as table flags
    pub const HAS_ON_REMOVE: Self = Self(1 << 17);
    pub const HAS_ON_SET: Self = Self(1 << 18);
    pub const HAS_ON_TABLE_CREATE: Self = Self(1 << 21);
    pub const HAS_ON_TABLE_DELETE: Self = Self(1 << 22);
    pub const IS_SPARSE: Self = Self(1 << 23);
    pub const IS_UNION: Self = Self(1 << 24);
    pub const EVENT_MASK: Self = Self(
        Self::HAS_ON_ADD.0 | Self::HAS_ON_REMOVE.0 | Self::HAS_ON_SET.0 |
        Self::HAS_ON_TABLE_CREATE.0 | Self::HAS_ON_TABLE_DELETE.0 | 
        Self::IS_SPARSE.0 | Self::IS_UNION.0
    );

    // Special flag
    pub const MARKED_FOR_DELETE: Self = Self(1 << 30);

    /// Returns an empty set of flags.
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
}

impl BitOr for ComponentFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ComponentFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ComponentFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for ComponentFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitXor for ComponentFlags {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for ComponentFlags {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for ComponentFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}
