use std::{ptr::NonNull, rc::Rc};

use crate::type_meta::TypeMeta;

#[derive(Debug)]
pub(crate) struct Blob {
    data: NonNull<u8>,
    meta: Rc<TypeMeta>,
}

impl Blob {
    /// Creates a new column with a dangling pointer (no allocation).
    pub(crate) fn new(meta: Rc<TypeMeta>) -> Self {
        Self { data: meta.dangling, meta }
    }

    /// Whether this column stores ZSTs.
    #[inline(always)]
    pub(crate) fn is_zst(&self) -> bool {
        self.meta.layout.size() == 0
    }

    /// Element stride in bytes. `meta.layout` is padded-to-align, so size == stride.
    #[inline(always)]
    pub(crate) fn stride(&self) -> usize {
        self.meta.layout.size()
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
    pub(crate) unsafe fn move_row_to(&self, src_row: u32, dst: &Blob, dst_row: u32) {
        debug_assert_eq!(self.stride(), dst.stride());
        unsafe {
            let src = self.row_ptr(src_row);
            let dst_ptr = dst.row_ptr(dst_row);
            dst_ptr.copy_from_nonoverlapping(src, self.stride());
        }
    }

    /// Drop the value at `row` in place.
    ///
    /// # Safety
    /// - `row` must be within the table's row count.
    /// - The value at `row` must be initialized and not already moved out.
    #[inline(always)]
    pub(crate) unsafe fn drop_row(&self, row: u32) {
        if let Some(dtor) = self.meta.dtor {
            unsafe { dtor(self.row_ptr(row)) };
        }
    }

    /// Reallocate backing storage from `old_cap` to `new_cap` elements.
    ///
    /// # Safety
    /// - `new_cap > old_cap`.
    /// - `old_cap` is this column's current capacity.
    pub(crate) unsafe fn realloc(&mut self, old_cap: usize, new_cap: usize) {
        if self.is_zst() {
            return;
        }

        let new_layout = self.meta.layout.repeat_packed(new_cap).unwrap();

        let ptr = unsafe {
            if old_cap == 0 {
                std::alloc::alloc(new_layout)
            } else {
                let old_layout = self.meta.layout.repeat_packed(old_cap).unwrap();
                std::alloc::realloc(self.data.as_ptr(), old_layout, new_layout.size())
            }
        };

        self.data = match NonNull::new(ptr) {
            Some(ptr) => ptr,
            None => std::alloc::handle_alloc_error(new_layout),
        };
    }

    /// Drop all elements in the column and deallocate.
    ///
    /// # Safety
    /// - `len` must be the actual number of initialized elements.
    /// - `cap` must be the current allocation capacity.
    pub(crate) unsafe fn destroy(&mut self, len: u32, cap: u32) {
        unsafe {
            if let Some(dtor) = self.meta.dtor {
                (0..len).for_each(|i| dtor(self.row_ptr(i)));
            }

            if !self.is_zst() {
                let layout = self.meta.layout.repeat_packed(cap as usize).unwrap();
                std::alloc::dealloc(self.data.as_ptr(), layout);
            }
        }
    }
}
