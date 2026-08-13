use std::{
    alloc::{self, Layout},
    ptr::NonNull,
};

#[derive(Debug)]
pub(crate) struct Block {
    data: NonNull<u8>,
    drop: Option<unsafe fn(ptr: NonNull<u8>)>,
    elem_layout: Layout,
}

impl Block {
    /// Creates a new column with a dangling pointer (no allocation).
    pub(crate) fn new(elem_layout: Layout, drop: Option<unsafe fn(ptr: NonNull<u8>)>) -> Self {
        debug_assert!(
            elem_layout == elem_layout.pad_to_align(),
            "Layout size must be a multiple of its alignment",
        );
        let data = elem_layout.dangling_ptr();
        Self { data, drop, elem_layout }
    }

    /// Whether this block stores ZSTs.
    #[inline(always)]
    pub(crate) fn is_zst(&self) -> bool {
        self.elem_layout.size() == 0
    }

    /// Element stride in bytes. `meta.layout` is padded-to-align, so size == stride.
    #[inline(always)]
    pub(crate) fn stride(&self) -> usize {
        self.elem_layout.size()
    }

    #[inline(always)]
    pub(crate) fn ptr(&self) -> NonNull<u8> {
        self.data
    }

    /// Byte pointer to the element at `row`. Internal; size read from meta..
    ///
    /// # Safety
    /// `row` must be within the table's current capacity.
    #[inline(always)]
    pub(crate) unsafe fn row_ptr(&self, row: u32) -> NonNull<u8> {
        unsafe { self.data.add(row as usize * self.stride()) }
    }

    /// Copy element bytes from `src_row` to `dst_row` within this column.
    /// Does not drop the destination; does not invalidate the source.
    ///
    /// # Safety
    /// Both rows must be within bounds.
    #[inline(always)]
    pub(crate) unsafe fn copy_row(&self, src_row: u32, dst_row: u32) {
        unsafe {
            let src = self.row_ptr(src_row);
            let dst = self.row_ptr(dst_row);
            dst.copy_from_nonoverlapping(src, self.stride());
        }
    }

    /// Copy this column's element from `src_row` into another column's `dst_row`.
    /// Both columns must store the same type (same component id). Used by table
    /// moves. Does not drop dst; does not invalidate src.
    ///
    /// # Safety
    /// - `self` and `dst` store the same type (equal `meta`).
    /// - `src_row` valid in `self`, `dst_row` valid in `dst`.
    #[inline(always)]
    pub(crate) unsafe fn move_row_to(&self, src_row: u32, dest: &Block, dst_row: u32) {
        debug_assert_eq!(self.stride(), dest.stride());
        unsafe {
            let src = self.row_ptr(src_row);
            let dst = dest.row_ptr(dst_row);
            dst.copy_from_nonoverlapping(src, self.stride());
        }
    }

    /// Drop the value at `row` in place.
    ///
    /// # Safety
    /// - `row` must be within the table's row count.
    /// - The value at `row` must be initialized and not already moved out.
    #[inline(always)]
    pub(crate) unsafe fn drop_row(&self, row: u32) {
        if let Some(drop) = self.drop {
            unsafe { drop(self.row_ptr(row)) };
        }
    }

    /// Reallocate backing storage from `old_cap` to `new_cap` elements.
    ///
    /// # Safety
    /// - `new_cap > old_cap`.
    /// - `old_cap` is this column's current capacity.
    pub(crate) unsafe fn realloc(&mut self, old_cap: u32, new_cap: u32) {
        if self.is_zst() {
            return;
        }

        let new_layout = self.elem_layout.repeat_packed(new_cap as usize).unwrap();

        let ptr = unsafe {
            if old_cap == 0 {
                alloc::alloc(new_layout)
            } else {
                let layout = self.elem_layout.repeat_packed(old_cap as usize).unwrap();
                alloc::realloc(self.data.as_ptr(), layout, new_layout.size())
            }
        };

        self.data = NonNull::new(ptr).unwrap_or_else(|| alloc::handle_alloc_error(new_layout));
    }

    /// Drop all elements in the column and deallocate.
    ///
    /// # Safety
    /// - `len` must be the actual number of initialized elements.
    /// - `cap` must be the current allocation capacity.
    pub(crate) unsafe fn drop(&mut self, len: u32, cap: u32) {
        unsafe {
            if let Some(drop) = self.drop {
                // self.drop is set to None for unwind safety.
                // This ensures elements are not dropped twice if drop panics.
                self.drop = None;
                (0..len).for_each(|i| drop(self.row_ptr(i)));
                self.drop = Some(drop);
            }

            if !self.is_zst() {
                let layout = self.elem_layout.repeat_packed(cap as usize).unwrap();
                alloc::dealloc(self.data.as_ptr(), layout);
            }
        }
    }
}
