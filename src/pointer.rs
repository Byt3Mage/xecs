use crate::{component::Component, types::type_info::TypeInfo};
use std::{any::TypeId, marker::PhantomData, ptr::NonNull};

pub struct Ptr<'a> {
    pub(crate) ptr: NonNull<u8>,
    pub(crate) type_id: TypeId,
    pub(crate) type_name: &'static str,
    _marker: PhantomData<&'a u8>,
}

impl<'a> Ptr<'a> {
    /// # Safety
    /// Callers must ensure that the type info matches the pointer
    pub(crate) unsafe fn new(ptr: NonNull<u8>, type_info: &TypeInfo) -> Self {
        Self {
            ptr,
            type_id: type_info.type_id,
            type_name: type_info.type_name,
            _marker: PhantomData,
        }
    }

    pub fn as_ref<C: Component>(self) -> Result<&'a C, Self> {
        if self.type_id == TypeId::of::<C>() {
            // SAFETY:
            // - ptr is NonNull
            // - we just checked that the type matches.
            Ok(unsafe { self.ptr.cast().as_ref() })
        } else {
            Err(self)
        }
    }
}

pub struct PtrMut<'a> {
    pub(crate) ptr: NonNull<u8>,
    pub(crate) type_id: TypeId,
    pub(crate) type_name: &'static str,
    _marker: PhantomData<&'a mut u8>,
}

impl<'a> PtrMut<'a> {
    /// # Safety
    /// Callers must ensure that the type info matches the pointer
    pub(crate) unsafe fn new(ptr: NonNull<u8>, type_info: &TypeInfo) -> Self {
        Self {
            ptr,
            type_id: type_info.type_id,
            type_name: type_info.type_name,
            _marker: PhantomData,
        }
    }

    /// Converts this pointer to a reference of the same type C
    /// Returns `Err(Self)` if there is a type mismatch.
    pub fn as_mut<C: Component>(self) -> Result<&'a mut C, Self> {
        if self.type_id == TypeId::of::<C>() {
            // SAFETY:
            // - ptr is NonNull
            // - we just checked that the type matches.
            Ok(unsafe { self.ptr.cast().as_mut() })
        } else {
            Err(self)
        }
    }
}

/// A newtype around [`NonNull`] that only allows conversion to read-only borrows or pointers.
///
/// This type can be thought of as the `*const T` to [`NonNull<T>`]'s `*mut T`.
#[repr(transparent)]
pub struct ConstNonNull<T: ?Sized>(NonNull<T>);

impl<T: ?Sized> ConstNonNull<T> {
    /// Creates a new `ConstNonNull` if `ptr` is non-null.
    pub fn new(ptr: *const T) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(Self)
    }

    /// Creates a new `ConstNonNull`.
    ///
    /// # Safety
    ///
    /// `ptr` must be non-null.
    pub const unsafe fn new_unchecked(ptr: *const T) -> Self {
        // SAFETY: This function's safety invariants are identical to `NonNull::new_unchecked`
        // The caller must satisfy all of them.
        unsafe { Self(NonNull::new_unchecked(ptr.cast_mut())) }
    }

    /// Returns a shared reference to the value.
    ///
    /// # Safety
    ///
    /// When calling this method, you have to ensure that all of the following is true:
    ///
    /// * The pointer must be properly aligned.
    ///
    /// * It must be "dereferenceable" in the sense defined in [the module documentation].
    ///
    /// * The pointer must point to an initialized instance of `T`.
    ///
    /// * You must enforce Rust's aliasing rules, since the returned lifetime `'a` is
    ///   arbitrarily chosen and does not necessarily reflect the actual lifetime of the data.
    ///   In particular, while this reference exists, the memory the pointer points to must
    ///   not get mutated (except inside `UnsafeCell`).
    #[inline]
    pub unsafe fn as_ref<'a>(&self) -> &'a T {
        // SAFETY: This function's safety invariants are identical to `NonNull::as_ref`
        // The caller must satisfy all of them.
        unsafe { self.0.as_ref() }
    }

    pub fn as_ptr(&self) -> *const T {
        self.0.as_ptr()
    }

    #[inline]
    pub const fn cast<U>(&self) -> ConstNonNull<U> {
        ConstNonNull(self.0.cast())
    }
}

impl<T: ?Sized> From<NonNull<T>> for ConstNonNull<T> {
    fn from(value: NonNull<T>) -> ConstNonNull<T> {
        ConstNonNull(value)
    }
}

impl<'a, T: ?Sized> From<&'a T> for ConstNonNull<T> {
    fn from(value: &'a T) -> ConstNonNull<T> {
        ConstNonNull(NonNull::from(value))
    }
}

impl<'a, T: ?Sized> From<&'a mut T> for ConstNonNull<T> {
    fn from(value: &'a mut T) -> ConstNonNull<T> {
        ConstNonNull(NonNull::from(value))
    }
}
