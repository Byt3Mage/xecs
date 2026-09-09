use std::num::NonZeroU32;

/// A 32-bit unsigned integer that is guaranteed not to be `u32::MAX`.
/// This enables `Option<NonMaxU32>` to be exactly 4 bytes.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct NonMaxU32(NonZeroU32);

impl NonMaxU32 {
    /// Creates a new `NonMaxU32` if the given value is not `u32::MAX`.
    #[inline]
    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value ^ u32::MAX) {
            Some(inner) => Some(Self(inner)),
            None => None,
        }
    }

    /// Creates a new `NonMaxU32` without checking the value.
    ///
    /// # Safety
    /// The value must not be `u32::MAX`. Passing `u32::MAX` results in undefined behavior.
    #[inline]
    pub const unsafe fn new_unchecked(value: u32) -> Self {
        Self(unsafe { NonZeroU32::new_unchecked(value ^ u32::MAX) })
    }

    /// Returns the primitive `u32` value.
    #[inline]
    pub const fn get(self) -> u32 {
        self.0.get() ^ u32::MAX
    }
}

impl std::fmt::Debug for NonMaxU32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.get(), f)
    }
}
