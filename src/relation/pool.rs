use std::ops::Range;

/// Block sizes are `1 << class` for `class` in `0..CLASSES`. Class 32 holds
/// a list of `u32::MAX` entries.
const CLASSES: usize = 33;

/// A non-empty list stored in a [`Pool`]: `len` entries starting at `offset`,
/// in a block of `1 << class` entries.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct Span {
    off: usize,
    len: u32,
    class: u8,
}

impl Span {
    /// Number of entries. Never zero.
    #[inline(always)]
    pub(crate) const fn len(self) -> u32 {
        self.len
    }

    #[inline(always)]
    const fn capacity(self) -> usize {
        (1usize << self.class)
    }

    #[inline(always)]
    const fn range(self) -> Range<usize> {
        self.off..(self.off + (self.len as usize))
    }
}

/// The result of [`Pool::swap_remove`].
#[derive(Copy, Clone, Debug)]
pub(crate) struct Removed<E> {
    /// The list after removal, or `None` when it became empty and its block
    /// was released.
    pub(crate) span: Option<Span>,
    /// The removed entry.
    pub(crate) entry: E,
    /// The entry moved into the removed position, when the removed entry was
    /// not last.
    pub(crate) moved: Option<E>,
}

/// Variable-length lists in power-of-two blocks drawn from one buffer.
///
/// Every element of `data` is initialized. Elements outside a live list's
/// `len` belong to no list. Released blocks are reused by later allocations
/// of the same class, and `data` never shrinks.
pub(crate) struct Pool<E: Copy> {
    data: Vec<E>,
    free: [Vec<usize>; CLASSES],
}

impl<E: Copy> Pool<E> {
    pub(crate) fn new() -> Self {
        Self {
            data: Vec::new(),
            free: std::array::from_fn(|_| Vec::new()),
        }
    }

    #[inline(always)]
    pub(crate) fn get(&self, span: Span) -> &[E] {
        &self.data[span.range()]
    }

    #[inline(always)]
    pub(crate) fn get_mut(&mut self, span: Span) -> &mut [E] {
        &mut self.data[span.range()]
    }

    /// Appends `entry` to `span`'s list, or starts a new list when `span` is
    /// `None`. Returns the list and the entry's position in it.
    ///
    /// A full block is replaced by one of twice the size. Positions of
    /// existing entries are unchanged.
    pub(crate) fn push(&mut self, span: Option<Span>, entry: E) -> (Span, u32) {
        let mut span = match span {
            Some(span) => span,
            None => Span { off: self.alloc(0, entry), len: 0, class: 0 },
        };

        if (span.len as usize) == span.capacity() {
            span = self.relocate(span, span.class + 1, entry);
        }

        let pos = span.len;
        self.data[span.off + (pos as usize)] = entry;
        // A list holds at most one entry per entity, so `len` stays within
        // `u32` and the class stays below `CLASSES`.
        span.len += 1;
        (span, pos)
    }

    /// Removes the entry at `pos` by moving the last entry into its place.
    ///
    /// The block is released when the list becomes empty, and replaced by one
    /// of half the size when the list falls to a quarter of its block.
    /// Positions of remaining entries are unchanged apart from the moved one.
    ///
    /// # Panics
    /// Panics when `pos` is not less than `span.len()`.
    pub(crate) fn swap_remove(&mut self, span: Span, pos: u32) -> Removed<E> {
        let list = self.get_mut(span);
        let entry = list[pos as usize];
        let last = (span.len - 1);

        let moved = if pos != last {
            let m = list[last as usize];
            list[pos as usize] = m;
            Some(m)
        } else {
            None
        };

        let span = Span { len: last, ..span };
        let span = if last == 0 {
            self.release(span.off, span.class);
            None
        } else if (last as usize) <= (span.capacity() >> 2) {
            let fill = self.data[span.off];
            Some(self.relocate(span, (span.class - 1), fill))
        } else {
            Some(span)
        };

        Removed { span, entry, moved }
    }

    /// Releases `span`'s block, discarding its entries.
    pub(crate) fn free(&mut self, span: Span) {
        self.release(span.off, span.class);
    }

    /// The offset of a block of `1 << class` entries. A released block of that
    /// class is reused when one exists. Otherwise `data` grows by one block,
    /// with every new element set to `fill`.
    fn alloc(&mut self, class: u8, fill: E) -> usize {
        match self.free[class as usize].pop() {
            Some(offset) => offset,
            None => {
                let offset = self.data.len();
                self.data.resize((offset + (1usize << class)), fill);
                offset
            }
        }
    }

    #[inline(always)]
    fn release(&mut self, offset: usize, class: u8) {
        self.free[class as usize].push(offset);
    }

    /// Copies `span`'s entries into a new block of `class` and releases the
    /// old block. `class` must hold `span.len()` entries.
    fn relocate(&mut self, span: Span, class: u8, fill: E) -> Span {
        let offset = self.alloc(class, fill);
        self.data.copy_within(span.range(), offset);
        self.release(span.off, span.class);
        Span { off: offset, len: span.len, class }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(pool: &mut Pool<u64>, n: u64) -> Span {
        let mut span = None;
        for v in 0..n {
            let (s, pos) = pool.push(span, v);
            assert_eq!(pos as u64, v);
            span = Some(s);
        }
        span.unwrap()
    }

    #[test]
    fn growth_preserves_positions() {
        let mut pool = Pool::new();
        let span = fill(&mut pool, 9);
        assert_eq!(span.len(), 9);
        assert_eq!(span.class, 4);
        assert_eq!(pool.get(span), &[0, 1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn swap_remove_reports_moved_entry() {
        let mut pool = Pool::new();
        let span = fill(&mut pool, 3);

        let r = pool.swap_remove(span, 0);
        assert_eq!(r.entry, 0);
        assert_eq!(r.moved, Some(2));
        let span = r.span.unwrap();
        assert_eq!(pool.get(span), &[2, 1]);

        let r = pool.swap_remove(span, 1);
        assert_eq!(r.entry, 1);
        assert_eq!(r.moved, None);
        let span = r.span.unwrap();

        let r = pool.swap_remove(span, 0);
        assert!(r.span.is_none());
    }

    #[test]
    fn shrinks_at_quarter() {
        let mut pool = Pool::new();
        let mut span = fill(&mut pool, 16);
        assert_eq!(span.class, 4);

        for _ in 0..12 {
            let last = (span.len() - 1);
            span = pool.swap_remove(span, last).span.unwrap();
        }
        assert_eq!(span.len(), 4);
        assert_eq!(span.class, 3);
        assert_eq!(pool.get(span), &[0, 1, 2, 3]);
    }

    #[test]
    fn released_blocks_are_reused() {
        let mut pool = Pool::new();
        let a = fill(&mut pool, 2);
        let used = pool.data.len();
        pool.free(a);

        let b = fill(&mut pool, 2);
        assert_eq!(pool.data.len(), used);
        assert_eq!(pool.get(b), &[0, 1]);
    }
}
