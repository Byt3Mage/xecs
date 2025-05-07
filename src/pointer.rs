use std::ptr::NonNull;

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
