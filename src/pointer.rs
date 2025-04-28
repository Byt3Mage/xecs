use std::{marker::PhantomData, mem::ManuallyDrop, ptr::NonNull};

/// Type-erased borrow of some unknown type.
///
/// This type tries to act "borrow-like" which means that:
/// - It should be considered immutable: its target must not be changed while this pointer is alive.
/// - It must always points to a valid value of whatever the pointee type is.
/// - The lifetime `'a` accurately represents how long the pointer is valid for.
/// - Must be sufficiently aligned for the unknown pointee type.
#[derive(Debug)]
#[repr(transparent)]
pub struct Ptr<'a> {
    inner: NonNull<u8>,
    phantom: PhantomData<&'a u8>,
}

impl<'a> Ptr<'a> {
    #[inline]
    pub unsafe fn new(ptr: NonNull<u8>) -> Self {
        Self {
            inner: ptr,
            phantom: PhantomData,
        }
    }

    /// Transforms this [`Ptr`] into a `&T` with the same lifetime
    ///
    /// # Safety
    /// `T` must be the erased pointee type for this [`Ptr`].
    #[inline]
    pub unsafe fn deref<T>(self) -> &'a T {
        // SAFETY: The caller ensures the pointee is of type `T` and the pointer can be dereferenced.
        unsafe { &*(self.as_ptr().cast::<T>()) }
    }

    /// Gets the underlying pointer, erasing the associated lifetime.
    ///
    /// If possible, it is strongly encouraged to use [`deref`](Self::deref) over this function,
    /// as it retains the lifetime.
    #[inline]
    pub fn as_ptr(self) -> *mut u8 {
        self.inner.as_ptr()
    }
}

/// Type-erased mutable borrow of some unknown type chosen when constructing this type.
///
/// This type tries to act "borrow-like" which means that:
/// - Pointer is considered exclusive and mutable. It cannot be cloned as this would lead to
///   aliased mutability.
/// - It must always points to a valid value of whatever the pointee type is.
/// - The lifetime `'a` accurately represents how long the pointer is valid for.
/// - Must be sufficiently aligned for the unknown pointee type.
///
/// It may be helpful to think of this type as similar to `&'a mut dyn Any` but without
/// the metadata and able to point to data that does not correspond to a Rust type.
#[derive(Debug)]
#[repr(transparent)]
pub struct PtrMut<'a> {
    inner: NonNull<u8>,
    phantom: PhantomData<&'a mut u8>,
}

impl<'a> PtrMut<'a> {
    /// Creates a new `PtrMut` from a NonNull ptr.
    ///
    /// # Safety
    /// The caller must ensure that the pointer is valid and properly aligned for the type it points to.
    #[inline]
    pub unsafe fn new(ptr: NonNull<u8>) -> Self {
        Self {
            inner: ptr,
            phantom: PhantomData,
        }
    }

    // Gets the underlying pointer, erasing the associated lifetime.
    ///
    /// If possible, it is strongly encouraged to use [`deref_mut`](Self::deref_mut) over
    /// this function, as it retains the lifetime.
    #[inline]
    pub fn as_ptr(&self) -> *mut u8 {
        self.inner.as_ptr()
    }

    /// Transforms this [`PtrMut`] into a `&mut T` with the same lifetime
    ///
    /// # Safety
    /// `T` must be the erased pointee type for this [`PtrMut`].
    pub unsafe fn deref_mut<T>(self) -> &'a mut T {
        // SAFETY: The caller ensures the pointee is of type `T` and the pointer can be dereferenced.
        unsafe { &mut *(self.as_ptr().cast::<T>()) }
    }

    /// Writes the value to the pointee
    ///
    /// # Safety
    /// `T` must be the erased pointee type for this [`PtrMut`].
    /// Caller must uphold the safety guarantees of [`write`](std::ptr::write).
    pub unsafe fn write<T>(self, value: T) {
        // SAFETY: The caller ensures the pointee is of type `T` and the pointer can be dereferenced.
        unsafe { self.as_ptr().cast::<T>().write(value) }
    }

    /// Gets an immutable reference from this mutable reference
    #[inline]
    pub fn as_ref(&self) -> Ptr<'a> {
        // SAFETY: The `PtrMut` type's guarantees about the validity of this pointer are a superset of `Ptr` s guarantees
        unsafe { Ptr::new(self.inner) }
    }

    #[inline]
    pub unsafe fn promote(self) -> OwningPtr<'a> {
        // SAFETY: The pointer is valid and the lifetime is accurate.
        OwningPtr {
            inner: self.inner,
            phantom: PhantomData,
        }
    }
}

impl<'a, T: ?Sized> From<&'a mut T> for PtrMut<'a> {
    #[inline]
    fn from(val: &'a mut T) -> Self {
        // SAFETY: The returned pointer has the same lifetime as the passed reference.
        // The reference is mutable, and thus will not alias.
        unsafe { Self::new(NonNull::from(val).cast()) }
    }
}

impl From<PtrMut<'_>> for NonNull<u8> {
    fn from(ptr: PtrMut<'_>) -> Self {
        ptr.inner
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct OwningPtr<'a> {
    inner: NonNull<u8>,
    phantom: PhantomData<&'a mut u8>,
}

impl<'a> OwningPtr<'a> {
    /// Creates a new `OwnedPtr` from a NonNull ptr.
    ///
    /// # Safety
    /// The caller must ensure that the pointer is valid and properly aligned for the type it points to.
    pub unsafe fn new(ptr: NonNull<u8>) -> Self {
        Self {
            inner: ptr,
            phantom: PhantomData,
        }
    }

    /// Gets the underlying pointer, erasing the associated lifetime.
    ///
    /// If possible, it is strongly encouraged to use [`deref_mut`](Self::deref_mut) over
    /// this function, as it retains the lifetime.
    #[inline]
    pub fn as_ptr(&self) -> *mut u8 {
        self.inner.as_ptr()
    }

    /// This exists mostly to reduce compile times;
    /// code is only duplicated per type, rather than per function called.
    ///
    /// # Safety
    ///
    /// Safety constraints of [`PtrMut::promote`] must be upheld.
    unsafe fn make_internal<T>(temp: &mut ManuallyDrop<T>) -> OwningPtr<'_> {
        // SAFETY: The constraints of `promote` are upheld by caller.
        unsafe { PtrMut::from(&mut *temp).promote() }
    }

    /// Consumes a value and creates an [`OwningPtr`] to it while ensuring a double drop does not happen.
    #[inline]
    pub fn make<T, F: FnOnce(OwningPtr<'_>) -> R, R>(val: T, f: F) -> R {
        // SAFETY: The value behind the pointer will not get dropped or observed later,
        // so it's safe to promote it to an owning pointer.
        let temp = &mut ManuallyDrop::new(val);
        f(unsafe { Self::make_internal(temp) })
    }

    /// Gets an immutable pointer from this owned pointer.
    #[inline]
    pub fn as_ref(&self) -> Ptr {
        // SAFETY: The `Owning` type's guarantees about the validity of this pointer are a superset of `Ptr` s guarantees
        unsafe { Ptr::new(self.inner) }
    }

    /// Gets a mutable pointer from this owned pointer.
    #[inline]
    pub fn as_mut(&mut self) -> PtrMut {
        // SAFETY: The `Owning` type's guarantees about the validity of this pointer are a superset of `Ptr` s guarantees
        unsafe { PtrMut::new(self.inner) }
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
