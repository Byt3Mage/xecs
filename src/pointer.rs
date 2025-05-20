use std::{marker::PhantomData, mem::ManuallyDrop, ops::DerefMut, ptr::NonNull};

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
    pub(crate) fn new(ptr: NonNull<u8>) -> Self {
        Self(ptr, PhantomData)
    }

    /// Acquires the underlying `*mut u8` ptr
    pub fn as_ptr(self) -> *mut u8 {
        self.0.as_ptr()
    }

    /// Converts this pointer to a mutable reference of type T.
    ///
    /// # Safety
    /// T must be the erased pointee for this [PtrMut].
    pub unsafe fn as_mut<T>(self) -> &'a mut T {
        // SAFETY:
        // Caller ensures that pointer is of type T
        unsafe { self.0.cast::<T>().as_mut() }
    }

    /// Transforms this [`PtrMut`] into an [`OwningPtr`]
    ///
    /// # Safety
    /// Caller must have right to drop or move out of [`PtrMut`].
    #[inline]
    pub unsafe fn to_owning(self) -> OwningPtr<'a> {
        OwningPtr(self.0, PhantomData)
    }
}

impl<'a, T: ?Sized> From<&'a mut T> for PtrMut<'a> {
    #[inline]
    fn from(val: &'a mut T) -> Self {
        Self::new(NonNull::from(val).cast())
    }
}

#[repr(transparent)]
pub struct OwningPtr<'a>(NonNull<u8>, PhantomData<&'a mut u8>);

impl<'a> OwningPtr<'a> {
    /// This exists mostly to reduce compile times;
    /// code is only duplicated per type, rather than per function called.
    ///
    /// # Safety
    ///
    /// Safety constraints of [`PtrMut::promote`] must be upheld.
    unsafe fn make_internal<T>(temp: &mut ManuallyDrop<T>) -> OwningPtr<'_> {
        // SAFETY: The constraints of `promote` are upheld by caller.
        unsafe { PtrMut::from(temp.deref_mut()).to_owning() }
    }

    /// Consumes a value and creates an [`OwningPtr`] to it while ensuring a double drop does not happen.
    #[inline]
    pub fn make<T, F: FnOnce(OwningPtr<'_>) -> R, R>(val: T, f: F) -> R {
        let mut val = ManuallyDrop::new(val);
        // SAFETY: The value behind the pointer will not get dropped or observed later,
        // so it's safe to promote it to an owning pointer.
        f(unsafe { Self::make_internal(&mut val) })
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
