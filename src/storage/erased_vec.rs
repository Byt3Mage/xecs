use std::{
    alloc::{self, Layout},
    ptr::{self, NonNull},
    rc::Rc,
};

use crate::{
    component::Component,
    pointer::{Ptr, PtrMut},
    types::type_info::TypeInfo,
};

/// Type-erased vector.
pub(crate) struct ErasedVec {
    data: NonNull<u8>,
    len: usize,
    cap: usize,
    type_info: Rc<TypeInfo>,
}

impl ErasedVec {
    pub(crate) fn new(type_info: Rc<TypeInfo>) -> Self {
        assert!(type_info.size() != 0, "can't create erased vec for ZSTs");
        Self {
            data: (type_info.dangling)(),
            len: 0,
            cap: 0,
            type_info,
        }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    fn grow(&mut self) {
        let (size, align) = self.type_info.size_align();

        if self.cap == 0 {
            let new_layout = Layout::from_size_align(size, align).unwrap();
            let new_ptr = unsafe { alloc::alloc(new_layout) };

            self.data = match NonNull::new(new_ptr) {
                Some(p) => p,
                None => alloc::handle_alloc_error(new_layout),
            };
            self.cap = 1;
        } else {
            let new_cap = 2 * self.cap;
            let old_layout = Layout::from_size_align(self.cap * size, align).unwrap();
            let new_layout = Layout::from_size_align(new_cap * size, align).unwrap();
            let old_ptr = self.data.as_ptr();
            let new_ptr = unsafe { alloc::realloc(old_ptr, old_layout, new_layout.size()) };

            self.data = match NonNull::new(new_ptr) {
                Some(p) => p,
                None => alloc::handle_alloc_error(new_layout),
            };
            self.cap = new_cap;
        }
    }

    /// Push an element into the vector
    pub(crate) fn push<T: 'static>(&mut self, elem: T) {
        debug_assert!(self.type_info.is::<T>(), "ErasedVec: type mismatch");

        if self.len == self.cap {
            self.grow();
        }

        let size = self.type_info.size();
        // SAFETY: We have checked that the type is correct and the index is within bounds.
        unsafe { self.data.add(size * self.len).cast().write(elem) };

        self.len += 1;
    }

    /// Returns a reference to the element at the given index
    /// without bounds or type checking.
    ///
    /// # Safety
    /// Caller must ensure that the index is within bounds and the type is correct.
    pub(crate) unsafe fn set_unchecked<C: Component>(&self, index: usize, elem: C) {
        unsafe {
            let ptr = self.data.add(self.type_info.size() * index).cast();
            let _ = ptr.replace(elem);
        }
    }

    /// Returns a reference to the element at the given index
    /// without bounds checking.
    ///
    /// # Panics
    /// Panics if the index is out of bounds or the type is incorrect.
    #[inline]
    pub(crate) fn set<C: Component>(&self, index: usize, elem: C) {
        debug_assert!(self.type_info.is::<C>(), "ErasedVec: type mismatch");
        assert!(self.len > index, "ErasedVec: index out of bounds");
        unsafe { self.set_unchecked(index, elem) }
    }

    /// Returns a reference to the element at the given index
    /// without bounds or type checking.
    ///
    /// # Safety
    /// Caller must ensure that the index is within bounds.
    #[inline]
    pub(crate) unsafe fn get_unchecked(&self, index: usize) -> Ptr {
        unsafe {
            let ptr = self.data.add(self.type_info.size() * index);
            Ptr::new(ptr, &self.type_info)
        }
    }

    /// Returns a mutable reference to the element at the given index
    /// without bounds checking.
    ///
    /// # Safety
    /// Caller must ensure that the index is within bounds and the type is correct.
    #[inline]
    pub(crate) unsafe fn get_unchecked_mut(&mut self, index: usize) -> PtrMut {
        unsafe {
            let ptr = self.data.add(self.type_info.size() * index);
            PtrMut::new(ptr, &self.type_info)
        }
    }

    /// Removes an element from the vector and returns it.
    /// The removed element is replaced by the last element of the vector.
    /// This does not preserve ordering of the remaining elements, but is O(1).
    ///
    /// # Panics
    /// Panics if the index is out of bounds.
    pub(crate) fn swap_remove<C: Component>(&mut self, index: usize) -> C {
        debug_assert!(self.type_info.is::<C>(), "ErasedVec: type mismatch");

        let len = self.len();

        if index >= len {
            panic!("swap_remove index (is {index}) should be < len (is {len})");
        }

        unsafe {
            // We replace self[index] with the last element. Note that if the
            // bounds check above succeeds there must be a last element (which
            // can be self[index] itself).
            let size = self.type_info.size();
            let value = self.data.add(size * index).cast().read();
            let base_ptr = self.data.as_ptr();

            ptr::copy(
                base_ptr.add(size * (len - 1)),
                base_ptr.add(size * index),
                size,
            );

            self.len -= 1;
            value
        }
    }
}

impl Drop for ErasedVec {
    fn drop(&mut self) {
        if self.cap != 0 {
            let (size, align) = self.type_info.size_align();
            unsafe {
                if let Some(drop_fn) = self.type_info.drop_fn {
                    let mut ptr = self.data;
                    for _ in 0..self.len {
                        (drop_fn)(ptr.add(size));
                        ptr = ptr.add(size);
                    }
                }

                let layout = Layout::from_size_align(size * self.cap, align).unwrap();
                alloc::dealloc(self.data.as_ptr(), layout);
            }
        }
    }
}
