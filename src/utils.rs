use core::fmt;
use std::{cmp::Ordering, num::NonZeroU32, sync::atomic::AtomicU32};

/// A non-max 32-bit id
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NonMaxU32(NonZeroU32);

impl NonMaxU32 {
    /// Stores `!n`, so `u32::MAX` maps to zero and is rejected; this gives
    /// `Option<NonMaxU32>` the same size as `u32`.
    #[inline(always)]
    pub const fn new(n: u32) -> Option<Self> {
        match NonZeroU32::new(!n) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }

    /// # Safety
    /// Caller must ensure that `n` is not equal to u32::MAX
    #[inline(always)]
    pub const unsafe fn new_unchecked(n: u32) -> Self {
        Self(unsafe { ::core::num::NonZeroU32::new_unchecked(n ^ u32::MAX) })
    }

    #[inline(always)]
    pub const fn get(self) -> u32 {
        !self.0.get()
    }
}

impl PartialOrd for NonMaxU32 {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NonMaxU32 {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        self.get().cmp(&other.get())
    }
}

impl fmt::Debug for NonMaxU32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.get(), f)
    }
}

/// The id slot of one static component, assigned by `static_components`.
#[derive(Debug)]
#[repr(transparent)]
pub struct KeySlot(AtomicU32);

impl KeySlot {
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(AtomicU32::new(u32::MAX))
    }

    #[inline(always)]
    pub(crate) fn load(&self) -> u32 {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[inline(always)]
    pub(crate) fn store(&self, slot: u32) {
        self.0.store(slot, std::sync::atomic::Ordering::Relaxed);
    }
}
