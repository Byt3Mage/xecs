use crate::type_info::TypeInfo;
use std::{
    alloc::{self, Layout},
    ptr::{self, NonNull},
};

/// Type-erased vector.
pub(crate) struct ErasedVec {
    data: NonNull<u8>,
    len: usize,
    cap: usize,
    type_info: &'static TypeInfo,
}

impl ErasedVec {
    pub(crate) fn new(type_info: &'static TypeInfo) -> Self {
        assert!(type_info.size() != 0, "can't create erased vec for ZSTs");
        Self {
            data: NonNull::dangling(),
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
    ///
    /// # Panics
    /// Panics if `elem` is not the correct type.
    pub(crate) fn push<T: 'static>(&mut self, elem: T) {
        assert!(self.type_info.is::<T>(), "type mismatch");

        if self.len == self.cap {
            self.grow();
        }

        let size = self.type_info.size();
        // SAFETY: We have checked that the type is correct and the index is within bounds.
        unsafe { self.data.add(size * self.len).cast().write(elem) };

        self.len += 1;
    }

    pub(crate) fn pop<T: 'static>(&mut self) -> Option<T> {
        assert!(self.type_info.is::<T>(), "type mismatch");

        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            let size = self.type_info.size();
            unsafe { Some(self.data.add(size * self.len).cast().read()) }
        }
    }

    /// Returns a reference to the element at the given index
    /// without bounds or type checking.
    ///
    /// # Safety
    /// Caller must ensure that the index is within bounds and the type is correct.
    pub(crate) unsafe fn set_unchecked<T: 'static>(&self, index: usize, elem: T) {
        debug_assert!(self.type_info.is::<T>(), "type mismatch");
        debug_assert!(self.len > index, "index out of bounds");
        unsafe {
            let ptr = self.data.add(self.type_info.size() * index).cast();
            let _ = ptr.replace(elem);
        }
    }

    /// Returns a reference to the element at the given index
    /// without bounds or type checking.
    ///
    /// # Panics
    /// Panics if the index is out of bounds or the type is incorrect.
    #[inline]
    pub(crate) fn set<T: 'static>(&self, index: usize, elem: T) {
        assert!(self.type_info.is::<T>(), "type mismatch");
        assert!(self.len > index, "index out of bounds");
        unsafe { self.set_unchecked(index, elem) }
    }

    /// Returns a reference to the element at the given index
    /// without bounds or type checking.
    ///
    /// # Safety
    /// Caller must ensure that the index is within bounds and the type is correct.
    #[inline]
    pub(crate) unsafe fn get_unchecked<T: 'static>(&self, index: usize) -> &T {
        debug_assert!(self.type_info.is::<T>(), "type mismatch");
        debug_assert!(self.len > index, "index out of bounds");
        unsafe { self.data.add(self.type_info.size() * index).cast().as_ref() }
    }

    /// Returns a reference to the element at the given index.
    /// Returns `None` if the index is out of bounds or the type is incorrect.
    #[inline]
    pub(crate) fn get<T: 'static>(&self, index: usize) -> Option<&T> {
        if self.type_info.is::<T>() && index < self.len {
            Some(unsafe { self.get_unchecked(index) })
        } else {
            None
        }
    }

    /// Returns a mutable reference to the element at the given index
    /// without bounds or type checking.
    ///
    /// # Safety
    /// Caller must ensure that the index is within bounds and the type is correct.
    #[inline]
    pub(crate) unsafe fn get_unchecked_mut<T: 'static>(&mut self, index: usize) -> &mut T {
        debug_assert!(self.type_info.is::<T>(), "type mismatch");
        debug_assert!(self.len > index, "index out of bounds");
        unsafe { self.data.add(self.type_info.size() * index).cast().as_mut() }
    }

    /// Returns a mutable reference to the element at the given index.
    /// Returns `None` if the index is out of bounds or the type is incorrect.
    #[inline]
    pub(crate) fn get_mut<T: 'static>(&mut self, index: usize) -> Option<&mut T> {
        if self.type_info.is::<T>() && index < self.len {
            Some(unsafe { self.get_unchecked_mut(index) })
        } else {
            None
        }
    }

    /// Removes an element from the vector and returns it.
    /// The removed element is replaced by the last element of the vector.
    /// This does not preserve ordering of the remaining elements, but is O(1).
    ///
    /// # Panics
    /// Panics if the index is out of bounds or the type is incorrect.
    pub(crate) fn swap_remove<T: 'static>(&mut self, index: usize) -> T {
        assert!(self.type_info.is::<T>(), "type mismatch");

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
            let drop_fn = self.type_info.drop_fn;
            let mut ptr = self.data.as_ptr();

            unsafe {
                for _ in 0..self.len {
                    drop_fn(ptr.add(size));
                    ptr = ptr.add(size);
                }

                let layout = Layout::from_size_align(size * self.cap, align).unwrap();
                alloc::dealloc(self.data.as_ptr(), layout);
            }
        }
    }
}
