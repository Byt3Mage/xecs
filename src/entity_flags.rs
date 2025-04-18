use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};


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