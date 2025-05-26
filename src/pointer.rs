use std::{marker::PhantomData, ptr::NonNull};

/// Typed-erased pointer with lifetime tracking.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Ptr<'a>(NonNull<u8>, PhantomData<&'a u8>);

impl<'a> Ptr<'a> {
    pub(crate) fn new(ptr: NonNull<u8>) -> Self {
        Self(ptr, PhantomData)
    }

    pub fn as_ptr(self) -> *const u8 {
        self.0.as_ptr()
    }

    /// Converts this pointer to a reference of type T
    ///
    /// # Safety
    /// T must be the erased pointee for this [Ptr].
    pub unsafe fn as_ref<T>(self) -> &'a T {
        // SAFETY:
        // Caller ensures that pointer is of type T
        unsafe { self.0.cast::<T>().as_ref() }
    }
}

/// Typed-erased mutable pointer with lifetime tracking.
#[repr(transparent)]
pub struct PtrMut<'a>(NonNull<u8>, PhantomData<&'a mut u8>);

impl<'a> PtrMut<'a> {
    #[inline]
    pub(crate) fn new(ptr: NonNull<u8>) -> Self {
        Self(ptr, PhantomData)
    }

    /// Acquires the underlying `*mut u8` ptr
    pub fn as_ptr(self) -> *mut u8 {
        self.0.as_ptr()
    }

    /// Converts this pointer to a reference of type T.
    ///
    /// # Safety
    /// - T must be the erased pointee for this [PtrMut].
    /// - The memory pointed to by this [PtrMut] must be initialized.
    #[inline]
    pub unsafe fn as_ref<T>(self) -> &'a T {
        unsafe { self.0.cast::<T>().as_ref() }
    }

    /// Converts this pointer to a mutable reference of type T.
    ///
    /// # Safety
    /// - T must be the erased pointee for this [PtrMut].
    /// - The memory pointed to by this [PtrMut] must be initialized.
    #[inline]
    pub unsafe fn as_mut<T>(self) -> &'a mut T {
        unsafe { self.0.cast::<T>().as_mut() }
    }

    /// Replaces the value pointed to by this [PtrMut].
    ///
    /// # Safety
    /// - T must be the erased pointee for this [PtrMut].
    /// - The memory pointed to by this [PtrMut] must be initialized.
    #[inline]
    pub unsafe fn replace<T>(self, src: T) -> T {
        unsafe { self.0.cast::<T>().replace(src) }
    }
}

impl<'a, T: ?Sized> From<&'a mut T> for PtrMut<'a> {
    #[inline]
    fn from(val: &'a mut T) -> Self {
        Self::new(NonNull::from(val).cast())
    }
}
