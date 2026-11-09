/// A position in a [`Slots`] of `N` positions. Always less than `N`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SlotIdx<const N: usize>(u32);

impl<const N: usize> SlotIdx<N> {
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
pub(crate) struct Slots<T: Copy, const N: usize> {
    items: [Option<T>; N],
}

impl<T: Copy, const N: usize> Slots<T, N> {
    /// All positions empty. Fails to compile when `N` is zero or exceeds
    /// `u32::MAX`.
    #[inline(always)]
    pub(crate) const fn new() -> Self {
        const {
            assert!(N > 0, "slots need at least one position");
            assert!(N <= (u32::MAX as usize), "slot count exceeds u32");
        };
        Self { items: [None; N] }
    }

    #[inline(always)]
    pub(crate) fn get(&self, i: SlotIdx<N>) -> Option<&T> {
        self.items[i.index()].as_ref()
    }

    #[inline(always)]
    pub(crate) fn get_mut(&mut self, i: SlotIdx<N>) -> Option<&mut T> {
        self.items[i.index()].as_mut()
    }

    /// Writes `entry` at `i`, returning the entry it replaced.
    #[inline(always)]
    pub(crate) fn replace(&mut self, i: SlotIdx<N>, entry: T) -> Option<T> {
        self.items[i.index()].replace(entry)
    }

    /// Empties `i`, returning the entry it held.
    #[inline(always)]
    pub(crate) fn take(&mut self, i: SlotIdx<N>) -> Option<T> {
        self.items[i.index()].take()
    }

    /// Writes `entry` at the lowest empty position and returns that position.
    /// Returns `None` without writing when every position is occupied.
    pub(crate) fn push(&mut self, entry: T) -> Option<SlotIdx<N>> {
        let i = self.items.iter().position(Option::is_none)?;
        self.items[i] = Some(entry);
        Some(SlotIdx(i as u32))
    }

    /// Number of occupied positions.
    pub(crate) fn len(&self) -> u32 {
        self.items.iter().filter(|e| e.is_some()).count() as u32
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.items.iter().all(Option::is_none)
    }

    /// Occupied positions in ascending order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (SlotIdx<N>, &T)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.as_ref().map(|e| (SlotIdx(i as u32), e)))
    }

    /// Occupied positions in ascending order.
    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (SlotIdx<N>, &mut T)> {
        self.items
            .iter_mut()
            .enumerate()
            .filter_map(|(i, e)| e.as_mut().map(|e| (SlotIdx(i as u32), e)))
    }

    /// The lowest occupied position whose entry satisfies `pred`.
    pub(crate) fn position(&self, mut pred: impl FnMut(&T) -> bool) -> Option<SlotIdx<N>> {
        self.iter().find(|(_, e)| pred(e)).map(|(i, _)| i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_fills_lowest_hole() {
        let mut s = Slots::<u64, 3>::new();
        assert_eq!(s.push(10), SlotIdx::new(0));
        assert_eq!(s.push(11), SlotIdx::new(1));
        assert_eq!(s.push(12), SlotIdx::new(2));
        assert_eq!(s.push(13), None);

        assert_eq!(s.take(SlotIdx::at::<1>()), Some(11));
        assert_eq!(s.len(), 2);
        assert_eq!(s.get(SlotIdx::at::<2>()), Some(&12));

        assert_eq!(s.push(14), SlotIdx::new(1));
        assert_eq!(s.get(SlotIdx::at::<1>()), Some(&14));
    }

    #[test]
    fn replace_and_iterate() {
        let mut s = Slots::<u64, 4>::new();
        assert_eq!(s.replace(SlotIdx::at::<2>(), 7), None);
        assert_eq!(s.replace(SlotIdx::at::<2>(), 8), Some(7));
        s.replace(SlotIdx::at::<0>(), 1);

        let seen: Vec<(u32, u64)> = s.iter().map(|(i, &e)| (i.get(), e)).collect();
        assert_eq!(seen, [(0, 1), (2, 8)]);
        assert_eq!(s.position(|&e| e == 8), SlotIdx::new(2));

        s.take(SlotIdx::at::<0>());
        s.take(SlotIdx::at::<2>());
        assert!(s.is_empty());
    }

    #[test]
    fn index_bounds() {
        assert!(SlotIdx::<2>::new(1).is_some());
        assert!(SlotIdx::<2>::new(2).is_none());
    }
}
