/// A position in a [`Slots`] of `N` positions. Always less than `N`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(transparent)]
pub struct Slot<const N: usize>(u32);

impl<const N: usize> Slot<N> {
    /// Position `I`. Fails to compile when `I` is not less than `N`.
    #[inline(always)]
    pub const fn at<const I: usize>() -> Self {
        const {
            assert!(I < N, "slot index out of range");
            assert!(I <= (u32::MAX as usize), "slot index exceeds u32");
        };
        Self(I as u32)
    }

    /// Position `i`, or `None` when `i` is not less than `N`.
    #[inline(always)]
    pub const fn new(i: u32) -> Option<Self> {
        if (i as usize) < N { Some(Self(i)) } else { None }
    }

    #[inline(always)]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[inline(always)]
    const fn index(self) -> usize {
        self.0 as usize
    }
}

/// `N` positions, each empty or holding one entry. Removing an entry empties
/// its position and moves no other entry.
#[derive(Copy, Clone, Debug)]
pub struct Slots<T: Copy, const N: usize> {
    pub(crate) items: [Option<T>; N],
}

impl<T: Copy, const N: usize> Slots<T, N> {
    /// All positions empty. Fails to compile when `N` is zero or exceeds
    /// `u32::MAX`.
    #[inline(always)]
    pub(crate) const fn new() -> Self {
        const {
            assert!(N > 1, "slots need at least two positions");
            assert!(N <= (u32::MAX as usize), "slot count exceeds u32");
        };
        Self { items: [None; N] }
    }

    pub(crate) fn with_entry(slot: Slot<N>, value: T) -> Self {
        let mut slots = Self::new();
        slots.items[slot.index()] = Some(value);
        slots
    }

    #[inline(always)]
    pub(crate) fn get_mut(&mut self, i: Slot<N>) -> Option<&mut T> {
        self.items[i.index()].as_mut()
    }

    /// Writes `entry` at `i`, returning the entry it replaced.
    #[inline(always)]
    pub(crate) fn replace(&mut self, i: Slot<N>, entry: T) -> Option<T> {
        self.items[i.index()].replace(entry)
    }

    /// Empties `i`, returning the entry it held.
    #[inline(always)]
    pub(crate) fn take(&mut self, i: Slot<N>) -> Option<T> {
        self.items[i.index()].take()
    }

    /// Number of occupied positions.
    pub(crate) fn len(&self) -> u32 {
        self.items.iter().filter(|e| e.is_some()).count() as u32
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().filter_map(Option::as_ref)
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.items.iter_mut().filter_map(Option::as_mut)
    }
}

impl<T: Copy, const N: usize> std::ops::Index<Slot<N>> for Slots<T, N> {
    type Output = T;
    #[inline(always)]
    fn index(&self, i: Slot<N>) -> &T {
        self.items[i.index()].as_ref().unwrap()
    }
}

impl<T: Copy, const N: usize> std::ops::IndexMut<Slot<N>> for Slots<T, N> {
    #[inline(always)]
    fn index_mut(&mut self, i: Slot<N>) -> &mut Self::Output {
        self.items[i.index()].as_mut().unwrap()
    }
}
