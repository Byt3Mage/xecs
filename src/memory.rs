use std::{cmp::max, marker::PhantomData, ops::{Deref, DerefMut}, ptr::NonNull};

const MIN_CAPACITY: usize = 1;

pub(crate) struct Ref<T> {
    ptr: NonNull<T>,
}

impl <T> Deref for Ref< T> {
    type Target = T;

    fn deref<'a>(&'a self) -> &'a Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl <T> DerefMut for Ref<T> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

pub(crate) struct BlockArena<T> {
    current: Vec<T>,
    rest: Vec<Vec<T>>,
}

impl<T> BlockArena<T> {
    pub fn new(capacity: usize) -> Self {
        Self { 
            current: Vec::with_capacity(max(capacity, MIN_CAPACITY)),
            rest: Vec::new(),
        }
    }

    pub fn allocate(&mut self, value: T) -> Ref<T> {
        if self.current.len() == self.current.capacity() {
            let new_capacity = max(self.current.capacity() * 2, MIN_CAPACITY);
            let mut new_block = Vec::with_capacity(new_capacity);
            
            new_block.push(value);
            let ptr = unsafe { NonNull::new_unchecked(new_block.as_mut_ptr()) };
            
            let prev = std::mem::replace(&mut self.current, new_block);
            self.rest.push(prev);
            
            Ref{ ptr}
        } else {
            let len = self.current.len();
            self.current.push(value);
            let ptr = unsafe { NonNull::new_unchecked(self.current.as_mut_ptr().add(len)) };
            Ref{ ptr }
        }
    }
}