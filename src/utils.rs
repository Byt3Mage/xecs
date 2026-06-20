use std::ptr::NonNull;

pub struct ConstNonNull<T>(NonNull<T>);

impl<T> ConstNonNull<T> {
    #[inline(always)]
    pub fn new(ptr: NonNull<T>) -> Self {
        Self(ptr)
    }

    pub fn from_ref(r: &T) -> Self {
        Self(NonNull::from_ref(r))
    }

    pub unsafe fn as_ref<'a>(&self) -> &'a T {
        unsafe { self.0.as_ref() }
    }

    pub fn add(&self, count: usize) -> Self {
        Self(unsafe { self.0.add(count) })
    }
}
