use core::num::NonZeroU32;

pub(crate) mod allocator;

/// Packed 64-bit identifier.
/// 
/// Layout (MSB → LSB): \[version: 32][index: 32\]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C, align(8))]
pub struct Id {
    pub index: u32,
    pub version: NonZeroU32,
}

const _: () = assert!(size_of::<Id>() == 8);
const _: () = assert!(size_of::<Option<Id>>() == 8);

impl Id {
    #[inline(always)]
    pub const fn new(index: u32, version: NonZeroU32) -> Self {
        Self { index, version }
    }

    #[inline(always)]
    pub(crate) const fn next_version(self) -> Self {
        Self {
            index: self.index,
            version: match NonZeroU32::new(self.version.get().wrapping_add(1)) {
                Some(v) => v,
                None => NonZeroU32::MIN,
            },
        }
    }

    /// The id as one integer: generation in the high 32 bits, index in the low 32.
    /// Identical on every platform, for serialization and FFI.
    #[inline(always)]
    pub const fn to_bits(self) -> u64 {
        ((self.version.get() as u64) << 32) | (self.index as u64)
    }

    /// The id encoded by `bits`, or `None` when its generation is zero.
    #[inline(always)]
    pub const fn from_bits(bits: u64) -> Option<Self> {
        match NonZeroU32::new((bits >> 32) as u32) {
            Some(version) => Some(Self { index: (bits as u32), version }),
            None => None,
        }
    }
}

impl core::hash::Hash for Id {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.to_bits());
    }
}

impl core::fmt::Display for Id {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ID#{}v{}", self.index, self.version)
    }
}

impl core::fmt::Debug for Id {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self, f)
    }
}
