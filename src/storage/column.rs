use std::{ptr::NonNull, rc::Rc};

use crate::{id::Id, storage::borrow::AtomicBorrow, type_meta::TypeMeta};

#[derive(Debug)]
pub(crate) struct Column {
    pub(super) id: Id,
    pub(super) data: NonNull<u8>,
    pub(super) type_meta: Rc<TypeMeta>,
    pub(super) borrow: AtomicBorrow,
}

impl Column {
    /// Creates a new column with a dangling pointer (no allocation).
    pub(crate) fn new(id: Id, type_meta: Rc<TypeMeta>) -> Self {
        Self {
            id,
            data: type_meta.dangling,
            type_meta,
            borrow: AtomicBorrow::new(),
        }
    }

    #[inline(always)]
    pub(crate) fn id(&self) -> Id {
        self.id
    }

    /// Whether this column stores ZSTs.
    #[inline(always)]
    pub(crate) fn is_zst(&self) -> bool {
        self.type_meta.layout.size() == 0
    }

    /// Whether this column stores ZSTs.
    #[inline(always)]
    pub(crate) fn data_size(&self) -> usize {
        self.type_meta.layout.size()
    }

    #[inline(always)]
    pub(super) fn as_ptr<T>(&self) -> *mut T {
        self.data.as_ptr().cast()
    }

    /// Raw pointer to the element at `row`.
    ///
    /// # Safety
    /// - `row` must be within the table's row count.
    #[inline(always)]
    pub(super) unsafe fn row_ptr(&self, row: usize, size: usize) -> NonNull<u8> {
        unsafe { self.data.add(row * size) }
    }

    #[inline]
    pub fn assert_type<T: 'static>(&self) {
        assert!(
            self.type_meta.is::<T>(),
            "column type mismatch: expected {}, requested {}",
            self.type_meta.name(),
            std::any::type_name::<T>(),
        );
    }

    /// Typed read of the element at `row`.
    ///
    /// # Safety
    /// - `row` must be within the table's row count.
    /// - `T` must be the type stored in this column.
    #[inline(always)]
    pub(crate) unsafe fn get<T: 'static>(&self, row: usize) -> &T {
        debug_assert!(self.type_meta.is::<T>());
        unsafe { self.row_ptr(row, self.data_size()).cast().as_ref() }
    }

    /// Typed mutable read of the element at `row`.
    ///
    /// # Safety
    /// - `row` must be within the table's row count.
    /// - `T` must be the type stored in this column.
    #[inline(always)]
    pub(crate) unsafe fn get_mut<T: 'static>(&self, row: usize) -> &mut T {
        debug_assert!(self.type_meta.is::<T>());
        unsafe { self.row_ptr(row, self.data_size()).cast().as_mut() }
    }

    /// Write a value at `row` without reading or dropping the old value.
    ///
    /// # Safety
    /// - `row` must be within the table's row capacity.
    /// - `T` must be the type stored in this column.
    /// - The slot at `row` must be uninitialized or already moved out.
    #[inline(always)]
    pub(crate) unsafe fn write<T: 'static>(&self, row: usize, val: T) {
        debug_assert!(self.type_meta.is::<T>());
        unsafe { self.row_ptr(row, self.data_size()).cast().write(val) };
    }

    /// Replace the value at `row`, returning the old value.
    ///
    /// # Safety
    /// - `row` must be within the table's row count.
    /// - `T` must be the type stored in this column.
    #[inline(always)]
    pub(crate) unsafe fn replace<T: 'static>(&self, row: usize, val: T) -> T {
        debug_assert!(self.type_meta.is::<T>());
        unsafe { self.row_ptr(row, self.data_size()).cast().replace(val) }
    }

    /// Copy raw bytes from `src_row` to `dst_row` within this column.
    /// Does not drop the destination. Does not invalidate the source.
    ///
    /// # Safety
    /// - Both rows must be within bounds.
    #[inline(always)]
    pub(crate) unsafe fn copy_row(&self, src_row: usize, dst_row: usize) {
        unsafe {
            let size = self.data_size();
            let src = self.row_ptr(src_row, size);
            let dst = self.row_ptr(dst_row, size);
            dst.copy_from_nonoverlapping(src, size);
        }
    }

    /// Drop the value at `row` in place.
    ///
    /// # Safety
    /// - `row` must be within the table's row count.
    /// - The value at `row` must be initialized and not already moved out.
    #[inline(always)]
    pub(crate) unsafe fn drop_row(&self, row: usize) {
        if let Some(dtor) = self.type_meta.dtor {
            unsafe { dtor(self.row_ptr(row, self.data_size())) };
        }
    }

    /// Reallocate the column's backing storage to `new_capacity` elements.
    ///
    /// # Safety
    /// - `new_cap` must be greater than the current capacity.
    /// - `old_cap` must be the current capacity.
    pub(crate) unsafe fn realloc(&mut self, old_cap: usize, new_cap: usize) {
        if self.is_zst() {
            return;
        }

        let new_layout = self.type_meta.layout.repeat_packed(new_cap).unwrap();

        let ptr = unsafe {
            if old_cap == 0 {
                std::alloc::alloc(new_layout)
            } else {
                let old_layout = self.type_meta.layout.repeat_packed(old_cap).unwrap();
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
            if let Some(dtor) = self.type_meta.dtor {
                let size = self.data_size();
                (0..len).for_each(|i| dtor(self.row_ptr(i, size)));
            }

            if !self.is_zst() {
                let layout = self.type_meta.layout.repeat_packed(cap).unwrap();
                std::alloc::dealloc(self.data.as_ptr(), layout);
            }
        }
    }
}
