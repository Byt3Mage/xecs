use std::{ptr::NonNull, rc::Rc};

use crate::{id::Id, type_meta::TypeMeta};

#[derive(Debug)]
pub(crate) struct Column {
    pub(super) id: Id,
    pub(super) data: NonNull<u8>,
    pub(super) meta: Rc<TypeMeta>,
}

impl Column {
    /// Creates a new column with a dangling pointer (no allocation).
    pub(crate) fn new(id: Id, meta: Rc<TypeMeta>) -> Self {
        Self { id, data: meta.dangling, meta }
    }

    #[inline(always)]
    pub(crate) fn id(&self) -> Id {
        self.id
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

    /// Byte pointer to the element at `row`. Internal; size read from meta..
    ///
    /// # Safety
    /// `row` must be within the table's current capacity.
    #[inline(always)]
    unsafe fn row_ptr(&self, row: usize) -> NonNull<u8> {
        unsafe { self.data.add(row * self.stride()) }
    }

    /// Typed shared read of the element at `row`.
    ///
    /// # Safety
    /// - `row` is within the table's row count.
    /// - `T` is the type stored in this column.
    /// - No `&mut` to this element exists for the returned lifetime.
    #[inline(always)]
    pub(crate) unsafe fn get<T: 'static>(&self, row: usize) -> &T {
        crate::validate::check_type::<T>(&self.meta);
        unsafe { self.row_ptr(row).cast().as_ref() }
    }

    /// Typed exclusive read of the element at `row`.
    ///
    /// # Safety
    /// - `row` is within the table's row count.
    /// - `T` is the type stored in this column.
    /// - No other borrow of this element exists for the returned lifetime.
    #[inline(always)]
    #[allow(clippy::mut_from_ref, reason = "Borrow checking is performed by callers ")]
    pub(crate) unsafe fn get_mut<T: 'static>(&self, row: usize) -> &mut T {
        crate::validate::check_type::<T>(&self.meta);
        unsafe { self.row_ptr(row).cast().as_mut() }
    }

    /// Borrow `len` initialized elements as `&[T]`.
    ///
    /// # Safety
    /// - `T` is the stored type.
    /// - No `&mut` to this column exists for the returned lifetime.
    #[inline(always)]
    pub(crate) unsafe fn slice<T: 'static>(&self, len: usize) -> &[T] {
        crate::validate::check_type::<T>(&self.meta);
        // SAFETY: data valid for len Ts (table invariant); aliasing upheld by caller.
        unsafe { std::slice::from_raw_parts(self.data.as_ptr().cast::<T>(), len) }
    }

    /// Borrow `len` initialized elements as `&mut [T]`.
    ///
    /// # Safety
    /// - `T` is the stored type.
    /// - No other borrow of this column exists for the returned lifetime.
    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    pub(crate) unsafe fn slice_mut<T: 'static>(&self, len: usize) -> &mut [T] {
        crate::validate::check_type::<T>(&self.meta);
        // SAFETY: exclusivity upheld by caller (query validation).
        unsafe { std::slice::from_raw_parts_mut(self.data.as_ptr().cast::<T>(), len) }
    }

    /// Write a value at `row` without reading or dropping the old value.
    ///
    /// # Safety
    /// - `row` must be within the table's row capacity.
    /// - `T` must be the type stored in this column.
    /// - The slot at `row` must be uninitialized or already moved out.
    #[inline(always)]
    pub(crate) unsafe fn write<T: 'static>(&self, row: usize, val: T) {
        crate::validate::check_type::<T>(&self.meta);
        unsafe { self.row_ptr(row).cast().write(val) };
    }

    /// Replace the value at `row`, returning the old value.
    ///
    /// # Safety
    /// - `row` must be within the table's row count.
    /// - `T` must be the type stored in this column.
    #[inline(always)]
    pub(crate) unsafe fn replace<T: 'static>(&self, row: usize, val: T) -> T {
        crate::validate::check_type::<T>(&self.meta);
        unsafe { self.row_ptr(row).cast().replace(val) }
    }

    /// Copy element bytes from `src_row` to `dst_row` within this column.
    /// Does not drop the destination; does not invalidate the source.
    ///
    /// # Safety
    /// Both rows must be within bounds.
    #[inline(always)]
    pub(crate) unsafe fn copy_row(&self, src_row: usize, dst_row: usize) {
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
    pub(crate) unsafe fn move_row_to(&self, src_row: usize, dst: &Column, dst_row: usize) {
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
    pub(crate) unsafe fn drop_row(&self, row: usize) {
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
    pub(crate) unsafe fn destroy(&mut self, len: usize, cap: usize) {
        if cap == 0 {
            return;
        }

        unsafe {
            if let Some(dtor) = self.meta.dtor {
                (0..len).for_each(|i| dtor(self.row_ptr(i)));
            }

            if !self.is_zst() {
                let layout = self.meta.layout.repeat_packed(cap).unwrap();
                std::alloc::dealloc(self.data.as_ptr(), layout);
            }
        }
    }
}
