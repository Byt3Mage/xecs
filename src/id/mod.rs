pub(crate) mod allocator;

/// Packed 64-bit identifier. Either a plain entity id (index + generation)
/// or a relationship pair (relation + target), distinguished by the kind tag
/// in the high 2 bits.
///
/// Layout (MSB → LSB): [generation: 32][index: 32]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Id(u64);

const INDEX_BITS: u32 = 32;
const INDEX_MASK: u64 = (1 << INDEX_BITS) - 1;

impl Id {
    /// Largest representable index
    pub const MAX_INDEX: u32 = u32::MAX - 1;
    pub const NULL: Self = Self::new(u32::MAX);

    #[inline(always)]
    pub const fn new(index: u32) -> Self {
        Self(index as u64)
    }

    #[inline(always)]
    pub(crate) const fn with_generation(index: u32, generation: u32) -> Self {
        Self(((generation as u64) << INDEX_BITS) | index as u64)
    }

    /// Creates a new `Id` from raw bits.
    #[inline(always)]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Converts the `Id` back to raw bits.
    #[inline(always)]
    pub const fn to_bits(self) -> u64 {
        self.0
    }

    #[inline(always)]
    pub const fn index(self) -> u32 {
        (self.0 & INDEX_MASK) as u32
    }

    #[inline(always)]
    pub const fn generation(self) -> u32 {
        (self.0 >> INDEX_BITS) as u32
    }

    /// Increments the version counter.
    #[inline(always)]
    pub(crate) const fn next_generation(self) -> Self {
        Self::with_generation(self.index(), self.generation().wrapping_add(1))
    }
}

impl core::fmt::Debug for Id {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ID #{}v{}", self.index(), self.generation())
    }
}

impl core::fmt::Display for Id {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ID #{}v{}", self.index(), self.generation())
    }
}
