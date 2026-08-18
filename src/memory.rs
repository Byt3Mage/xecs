use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc},
    ptr::NonNull,
};

pub type DropFn = Option<unsafe fn(ptr: NonNull<u8>)>;

/// How to interpret a `RawBlock`'s bytes: one element's padded layout
/// and its drop glue.
///
/// Held once by the structure that owns a block — a table's column map,
/// a relation index, a hierarchy — rather than stored per buffer, which
/// is what lets a per-source or per-tree block cost one pointer.
#[derive(Copy, Clone)]
pub(crate) struct RowMeta {
    /// Padded to alignment, so `size * n` addresses row `n`.
    layout: Layout,
    drop: DropFn,
}

impl RowMeta {
    pub(crate) fn new(layout: Layout, drop: DropFn) -> Self {
        Self { layout: layout.pad_to_align(), drop }
    }

    #[inline(always)]
    pub(crate) const fn stride(&self) -> usize {
        self.layout.size()
    }

    #[inline(always)]
    pub(crate) const fn align(&self) -> usize {
        self.layout.align()
    }

    #[inline(always)]
    pub(crate) const fn is_zst(&self) -> bool {
        self.layout.size() == 0
    }

    /// Layout of `n` contiguous rows.
    #[inline]
    fn array(&self, n: u32) -> Layout {
        // SAFETY: the alignment came from a valid layout, and the size
        // is a multiple of it because `layout` was padded.
        unsafe { Layout::from_size_align_unchecked(self.stride() * n as usize, self.align()) }
    }

    #[inline]
    fn dangling(&self) -> NonNull<u8> {
        self.layout.dangling_ptr()
    }
}

#[repr(transparent)]
pub(crate) struct RawBlock {
    ptr: NonNull<u8>,
}

impl RawBlock {
    /// An unallocated block. The pointer is aligned rather than merely
    /// non-null, so zero-sized reads and writes through it are
    /// well-formed without a special case anywhere else.
    #[inline]
    pub(crate) fn new(rm: RowMeta) -> Self {
        Self { ptr: rm.dangling() }
    }

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
    pub(crate) unsafe fn grow(&mut self, rm: RowMeta, old_cap: u32, new_cap: u32) {
        debug_assert!(new_cap > old_cap);

        if rm.is_zst() {
            return;
        }

        let layout = rm.array(new_cap);

        // SAFETY: `layout` has non-zero size — `new_cap` exceeds `old_cap`
        // and the stride is non-zero. On the realloc path `self.ptr` came
        // from `rm.array(old_cap)` per the caller's obligation.
        let ptr = unsafe {
            match old_cap {
                0 => alloc(layout),
                old => realloc(self.ptr.as_ptr(), rm.array(old), layout.size()),
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
    pub(crate) unsafe fn shrink(&mut self, rm: RowMeta, old_cap: u32, new_cap: u32) {
        debug_assert!(new_cap < old_cap);

        if rm.is_zst() || old_cap == 0 {
            return;
        }

        let old_layout = rm.array(old_cap);

        if new_cap == 0 {
            // SAFETY: `self.ptr` came from `old_layout`, and the caller
            // guarantees no live rows remain.
            unsafe { dealloc(self.ptr.as_ptr(), old_layout) };
            self.ptr = rm.dangling();
            return;
        }

        let layout = rm.array(new_cap);

        // SAFETY: `self.ptr` came from `old_layout`. Shrinking preserves
        // the first `new_cap` rows, all of which remain live.
        let ptr = unsafe { realloc(self.ptr.as_ptr(), old_layout, layout.size()) };
        self.ptr = NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(layout));
    }

    /// Release the allocation.
    ///
    /// # Safety
    /// `cap` is current, and no row in the block is live.
    pub(crate) unsafe fn dealloc(&mut self, rm: RowMeta, cap: u32) {
        if rm.is_zst() || cap == 0 {
            return;
        }
        unsafe { std::alloc::dealloc(self.ptr.as_ptr(), rm.array(cap)) };
        *self = Self::new(rm);
    }

    #[inline(always)]
    pub(crate) fn row(&self, rm: RowMeta, i: u32) -> NonNull<u8> {
        // SAFETY: callers keep `i` within capacity. For a zero stride
        // this is the aligned base, which is what a ZST access wants.
        unsafe { self.ptr.byte_add(i as usize * rm.stride()) }
    }

    /// # Safety
    /// `i` is within capacity and holds no live value; `T` matches `rm`.
    #[inline]
    pub(crate) unsafe fn write<T>(&mut self, rm: RowMeta, i: u32, value: T) {
        unsafe { self.row(rm, i).cast::<T>().write(value) };
    }

    /// # Safety
    /// `i` holds a live value; `T` matches `rm`.
    #[inline]
    pub(crate) unsafe fn read<T>(&self, rm: RowMeta, i: u32) -> &T {
        unsafe { self.row(rm, i).cast::<T>().as_ref() }
    }

    /// # Safety
    /// `i` holds a live value; `T` matches `rm`; no other reference to
    /// this row exists.
    #[inline]
    pub(crate) unsafe fn read_mut<T>(&mut self, rm: RowMeta, i: u32) -> &mut T {
        unsafe { self.row(rm, i).cast::<T>().as_mut() }
    }

    /// Move a value out.
    ///
    /// The row is dead afterwards: the caller owns the value and the
    /// block must not drop that row again.
    ///
    /// # Safety
    /// `i` holds a live value; `T` matches `rm`.
    #[inline]
    pub(crate) unsafe fn take<T>(&mut self, rm: RowMeta, i: u32) -> T {
        unsafe { self.row(rm, i).cast::<T>().read() }
    }

    /// # Safety
    /// `i` is within capacity and holds a live value.
    #[inline]
    pub(crate) unsafe fn drop_row(&self, rm: RowMeta, i: u32) {
        if let Some(drop) = rm.drop {
            unsafe { drop(self.row(rm, i)) };
        }
    }

    /// # Safety
    /// Every row in `range` is within capacity and live.
    #[inline]
    pub(crate) unsafe fn drop_rows(&self, rm: RowMeta, range: std::ops::Range<u32>) {
        if let Some(drop) = rm.drop {
            range.for_each(|i| unsafe { drop(self.row(rm, i)) });
        }
    }

    /// Relocate `count` rows within this block. Ranges may overlap.
    ///
    /// # Safety
    /// Both ranges are within capacity. Values are *moved*: the source
    /// rows are dead afterwards and must not be dropped, and the
    /// destination must have held nothing live.
    #[inline]
    pub(crate) unsafe fn shift(&mut self, rm: RowMeta, src: u32, dst: u32, count: u32) {
        if rm.is_zst() || count == 0 || src == dst {
            return;
        }
        unsafe {
            let src = self.row(rm, src);
            let dst = self.row(rm, dst);
            src.copy_to(dst, (count as usize) * rm.stride());
        };
    }

    /// Relocate `count` rows into another block.
    ///
    /// # Safety
    /// Both blocks use `rm`; both ranges are within their capacities;
    /// the blocks are distinct allocations. Same move obligations as
    /// `shift`.
    #[inline]
    pub(crate) unsafe fn move_row(&self, rm: RowMeta, src: u32, dest: &RawBlock, dst: u32, count: u32) {
        if rm.is_zst() || count == 0 {
            return;
        }
        unsafe {
            let src = self.row(rm, src);
            let dst = dest.row(rm, dst);
            src.copy_to_nonoverlapping(dst, (count as usize) * rm.stride());
        };
    }

    /// Drop the value at `at` and move the last one into its place,
    /// mirroring `Vec::swap_remove` for an owner whose length lives
    /// elsewhere.
    ///
    /// The drop happens first: the row must be dead before anything is
    /// moved onto it, or the incoming value would be destroyed instead.
    ///
    /// # Safety
    /// `at <= last`, both within capacity, and every row in `0..=last`
    /// is live. The owner's length becomes `last` on return, so that
    /// row is dead and must not be dropped again.
    #[inline]
    pub(crate) unsafe fn swap_remove(&mut self, rm: RowMeta, at: u32, last: u32) {
        debug_assert!(at <= last);
        unsafe {
            self.drop_row(rm, at);
            self.shift(rm, last, at, 1);
        }
    }
}
