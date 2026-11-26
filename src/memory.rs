use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc},
    ptr::NonNull,
};

/// The layout would exceed `isize::MAX` bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
#[error("layout size overflows isize")]
pub struct LayoutOverflow;

/// One element's layout, padded to its alignment so that `size * n`
/// addresses row `n`.
///
/// Held once by the structure that owns a block rather than per buffer,
/// so a block is one pointer.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct RowLayout(Layout);

impl RowLayout {
    #[inline(always)]
    pub const fn of<T: Copy + 'static>() -> Self {
        Self(Layout::new::<T>())
    }

    #[inline(always)]
    pub const fn inner(self) -> Layout {
        self.0
    }

    #[inline(always)]
    pub const fn size(&self) -> usize {
        self.0.size()
    }

    #[inline(always)]
    pub const fn align(&self) -> usize {
        self.0.align()
    }

    #[inline(always)]
    pub const fn is_zst(&self) -> bool {
        self.0.size() == 0
    }

    #[inline(always)]
    pub const fn extend(&self, next: Self) -> Result<(Self, usize), LayoutOverflow> {
        match self.0.extend(next.0) {
            Ok((l, off)) => Ok((Self(l), off)),
            Err(_) => Err(LayoutOverflow),
        }
    }

    #[inline(always)]
    pub const fn pad_to_align(&self) -> Self {
        Self(self.0.pad_to_align())
    }

    /// Layout of `n` contiguous rows.
    #[inline]
    pub const fn array(&self, n: u32) -> Result<Layout, LayoutOverflow> {
        match self.size().checked_mul(n as usize) {
            Some(size) => match Layout::from_size_align(size, self.align()) {
                Ok(layout) => Ok(layout),
                Err(_) => Err(LayoutOverflow),
            },
            None => Err(LayoutOverflow),
        }
    }

    #[inline(always)]
    pub(crate) fn dangling_ptr(&self) -> NonNull<u8> {
        self.0.dangling_ptr()
    }

    #[inline(always)]
    pub(crate) fn new_block(&self) -> RawBlock {
        RawBlock { ptr: self.0.dangling_ptr() }
    }
}

/// Untyped storage for rows of one `RowLayout`. Capacity is tracked by
/// the owner.
///
/// Elements are `Copy`, so rows are plain bytes. A row may be overwritten
/// or abandoned without any cleanup, and relocation is a byte copy.
#[repr(transparent)]
pub(crate) struct RawBlock {
    ptr: NonNull<u8>,
}

impl RawBlock {
    #[inline(always)]
    pub(crate) fn ptr(&self) -> NonNull<u8> {
        self.ptr
    }

    /// Grow to hold exactly `new_cap` rows. No-op for zero-sized elements.
    ///
    /// # Safety
    /// - `old_cap` is this block's current capacity, from a previous
    ///   `grow`/`shrink` on the same `rm`.
    /// - `new_cap > old_cap`.
    pub(crate) unsafe fn grow(&mut self, rl: RowLayout, old_cap: u32, new_cap: u32) {
        debug_assert!(new_cap > old_cap);

        if rl.is_zst() {
            return;
        }

        let layout = rl.array(new_cap).expect("capacity overflow");

        // SAFETY: `layout` has non-zero size — `new_cap` exceeds `old_cap`
        // and the stride is non-zero. On the realloc path `self.ptr` came
        // from `rm.array(old_cap)` per the caller's obligation.
        let ptr = unsafe {
            match old_cap {
                0 => alloc(layout),
                old => realloc(
                    self.ptr.as_ptr(),
                    rl.array(old).expect("capacity overflow"),
                    layout.size(),
                ),
            }
        };

        self.ptr = NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(layout));
    }

    /// Shrink to hold exactly `new_cap` rows, releasing the allocation
    /// when `new_cap` is zero. No-op for zero-sized elements.
    ///
    /// # Safety
    /// - `old_cap` is this block's current capacity, from a previous
    ///   `grow`/`shrink` on the same `rm`.
    /// - `new_cap < old_cap`.
    /// - No row at or past `new_cap` is live. Values there are lost
    ///   without being dropped.
    pub(crate) unsafe fn shrink(&mut self, rl: RowLayout, old_cap: u32, new_cap: u32) {
        debug_assert!(new_cap < old_cap);

        if rl.is_zst() || old_cap == 0 {
            return;
        }

        let old_layout = rl.array(old_cap).expect("capacity overflow");

        if new_cap == 0 {
            // SAFETY: `self.ptr` came from `old_layout`, and the caller
            // guarantees no live rows remain.
            unsafe { dealloc(self.ptr.as_ptr(), old_layout) };
            self.ptr = rl.dangling_ptr();
            return;
        }

        let layout = rl.array(new_cap).expect("capacity overflow");

        // SAFETY: `self.ptr` came from `old_layout`. Shrinking preserves
        // the first `new_cap` rows, all of which remain live.
        let ptr = unsafe { realloc(self.ptr.as_ptr(), old_layout, layout.size()) };
        self.ptr = NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(layout));
    }

    /// Release the allocation.
    ///
    /// # Safety
    /// `cap` is current, and no row in the block is live.
    pub(crate) unsafe fn dealloc(&mut self, rl: RowLayout, cap: u32) {
        if rl.is_zst() || cap == 0 {
            return;
        }
        unsafe { dealloc(self.ptr.as_ptr(), rl.array(cap).expect("capacity overflow")) };
        self.ptr = rl.dangling_ptr();
    }

    #[inline(always)]
    pub(crate) unsafe fn row(&self, size: usize, i: u32) -> NonNull<u8> {
        // SAFETY: callers keep `i` within capacity. For a zero stride
        // this is the aligned base, which is what a ZST access wants.
        unsafe { self.ptr.add(i as usize * size) }
    }

    /// Relocate `count` rows within this block. Ranges may overlap.
    ///
    /// # Safety
    /// Both ranges are within capacity. Values are *moved*: the source
    /// rows are dead afterwards and must not be dropped, and the
    /// destination must have held nothing live.
    #[inline]
    pub(crate) unsafe fn shift(&mut self, row_size: usize, src: u32, dst: u32, count: u32) {
        if row_size == 0 || count == 0 || src == dst {
            return;
        }

        unsafe {
            let src = self.row(row_size, src);
            let dst = self.row(row_size, dst);
            src.copy_to(dst, (count as usize) * row_size);
        };
    }

    /// Relocate `count` rows into another block.
    ///
    /// # Safety
    /// Both blocks use the same size; both ranges are within their capacities;
    /// the blocks are distinct allocations.
    #[inline]
    pub(crate) unsafe fn copy_rows(&self, row_size: usize, src: u32, dest: &RawBlock, dst: u32, count: u32) {
        if row_size == 0 || count == 0 {
            return;
        }
        unsafe {
            let src = self.row(row_size, src);
            let dst = dest.row(row_size, dst);
            src.copy_to_nonoverlapping(dst, (count as usize) * row_size);
        };
    }

    /// Overwrite row `at` with row `last`, mirroring `Vec::swap_remove`
    /// for an owner whose length is tracked elsewhere. The owner's length
    /// becomes `last`.
    ///
    /// # Safety
    /// `at <= last` and both are within capacity.
    #[inline]
    pub(crate) unsafe fn swap_remove(&mut self, row_size: usize, at: u32, last: u32) {
        debug_assert!(at <= last);
        unsafe { self.shift(row_size, last, at, 1) };
    }
}
