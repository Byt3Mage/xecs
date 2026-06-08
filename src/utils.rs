use std::{
    hash::{BuildHasher, Hasher},
    ptr::NonNull,
};

#[derive(Clone, Default)]
pub struct NoOpHash;

impl BuildHasher for NoOpHash {
    type Hasher = NoOpHasher;

    fn build_hasher(&self) -> Self::Hasher {
        NoOpHasher(0)
    }
}

pub struct NoOpHasher(u64);

impl Hasher for NoOpHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        // This should never be called by consumers.
        // Prefer to call write_u64 instead.
        self.0 = bytes.iter().fold(self.0, |hash, b| {
            hash.rotate_left(8).wrapping_add(*b as u64)
        });

        panic!("NoOpHasher only supports hashing with write_u64");
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0 = i;
    }
}

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
