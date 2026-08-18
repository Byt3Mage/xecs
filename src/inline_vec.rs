//! A small-vector: stores up to `N` elements inline, spills to the heap beyond that.
//!
//! Layout: `{ len: usize, cap: usize, data: union { inline: [MaybeUninit<T>; N],
//! heap: NonNull<T> } }`. The tag is `cap > N`. Keeping `cap` beside `len` means
//! `len()` is a load, and `push`'s bounds check is a single compare against a value
//! in the same cache line.
//!
//! ZSTs: never allocate, `cap` is pinned to `usize::MAX`, `len` is the only state.

use std::alloc::{self, Layout};
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::ops::{Deref, DerefMut};
use std::ptr::{self, NonNull};

union Data<T, const N: usize> {
    inline: ManuallyDrop<[MaybeUninit<T>; N]>,
    heap: NonNull<T>,
}

pub struct InlineVec<T, const N: usize> {
    data: Data<T, N>,
    len: usize,
    /// `> N` means spilled. For ZSTs, always `usize::MAX` (never "spilled" in the
    /// allocating sense; `spilled()` is guarded separately).
    cap: usize,
}

unsafe impl<T: Send, const N: usize> Send for InlineVec<T, N> {}
unsafe impl<T: Sync, const N: usize> Sync for InlineVec<T, N> {}

impl<T, const N: usize> InlineVec<T, N> {
    const IS_ZST: bool = mem::size_of::<T>() == 0;

    #[inline(always)]
    pub const fn new() -> Self {
        const { assert!(N > 0, "InlineVec requires N > 0") };

        Self {
            data: Data {
                inline: ManuallyDrop::new([const { MaybeUninit::uninit() }; N]),
            },
            len: 0,
            cap: if Self::IS_ZST { usize::MAX } else { N },
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        let mut v = Self::new();
        if !Self::IS_ZST && cap > N {
            v.grow_to(cap);
        }
        v
    }

    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        self.cap
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub fn is_spilled(&self) -> bool {
        !Self::IS_ZST && self.cap > N
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const T {
        unsafe {
            if !Self::IS_ZST && self.cap > N {
                self.data.heap.as_ptr()
            } else {
                // For ZSTs this is a dangling-but-aligned pointer into the union,
                // which is exactly what slice::from_raw_parts wants.
                self.data.inline.as_ptr() as *const T
            }
        }
    }

    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        unsafe {
            if !Self::IS_ZST && self.cap > N {
                self.data.heap.as_ptr()
            } else {
                ptr::addr_of_mut!(self.data.inline) as *mut T
            }
        }
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.as_ptr(), self.len) }
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }

    /// # Safety
    /// `i < self.len()`
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, i: usize) -> &T {
        debug_assert!(i < self.len);
        unsafe { &*self.as_ptr().add(i) }
    }

    /// # Safety
    /// `i < self.len()`
    #[inline(always)]
    pub unsafe fn get_unchecked_mut(&mut self, i: usize) -> &mut T {
        debug_assert!(i < self.len);
        unsafe { &mut *self.as_mut_ptr().add(i) }
    }

    #[inline(always)]
    pub fn get(&self, i: usize) -> Option<&T> {
        (i < self.len).then(|| unsafe { self.get_unchecked(i) })
    }

    #[inline(always)]
    pub fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        (i < self.len).then(|| unsafe { self.get_unchecked_mut(i) })
    }

    #[inline(always)]
    pub fn push(&mut self, value: T) {
        if self.len == self.cap {
            self.grow_one();
        }
        unsafe {
            ptr::write(self.as_mut_ptr().add(self.len), value);
            self.len += 1;
        }
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Option<T> {
        (self.len != 0).then(|| unsafe { self.pop_unchecked() })
    }

    /// # Safety
    /// `self.len() > 0`
    #[inline(always)]
    pub unsafe fn pop_unchecked(&mut self) -> T {
        debug_assert!(self.len > 0);
        unsafe {
            self.len -= 1;
            ptr::read(self.as_ptr().add(self.len))
        }
    }

    #[inline]
    pub fn insert(&mut self, index: usize, value: T) {
        assert!(index <= self.len, "insert index out of bounds");
        if self.len == self.cap {
            self.grow_one();
        }
        unsafe {
            let p = self.as_mut_ptr().add(index);
            ptr::copy(p, p.add(1), self.len - index);
            ptr::write(p, value);
            self.len += 1;
        }
    }

    #[inline]
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "remove index out of bounds");
        unsafe {
            self.len -= 1;
            let p = self.as_mut_ptr().add(index);
            let v = ptr::read(p);
            ptr::copy(p.add(1), p, self.len - index);
            v
        }
    }

    #[inline]
    pub fn swap_remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "swap_remove index out of bounds");
        unsafe {
            self.len -= 1;
            let base = self.as_mut_ptr();
            let v = ptr::read(base.add(index));
            ptr::copy(base.add(self.len), base.add(index), 1);
            v
        }
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        let elems: *mut [T] = self.as_mut_slice();
        unsafe {
            self.len = 0;
            ptr::drop_in_place(elems);
        }
    }

    /// # Safety
    /// `len <= self.capacity()` and elements `0..len` are initialized.
    #[inline(always)]
    unsafe fn set_len(&mut self, len: usize) {
        debug_assert!(len <= self.cap);
        self.len = len;
    }

    #[inline(always)]
    pub fn reserve(&mut self, additional: usize) {
        if additional > self.cap - self.len {
            let need = self.len.checked_add(additional).expect("capacity overflow");
            self.grow_to(need);
        }
    }

    #[cold]
    #[inline(never)]
    fn grow_one(&mut self) {
        // ZSTs have cap == usize::MAX, so len can never reach it without
        // overflowing the address space first. Unreachable in practice.
        debug_assert!(!Self::IS_ZST);
        let new_cap = if self.cap > N { self.cap.checked_mul(2).expect("capacity overflow") } else { (N * 2).max(4) };
        self.grow_to(new_cap);
    }

    #[cold]
    fn grow_to(&mut self, new_cap: usize) {
        if Self::IS_ZST {
            return;
        }
        debug_assert!(new_cap > self.cap);

        let new_layout = Layout::array::<T>(new_cap).expect("capacity overflow");
        assert!(new_layout.size() <= isize::MAX as usize, "allocation too large");

        unsafe {
            let new_ptr = if self.cap > N {
                let old_layout = Layout::array::<T>(self.cap).expect("layout was valid at allocation");
                alloc::realloc(self.data.heap.as_ptr() as *mut u8, old_layout, new_layout.size())
            } else {
                let p = alloc::alloc(new_layout);
                if !p.is_null() {
                    ptr::copy_nonoverlapping(self.data.inline.as_ptr() as *const T, p as *mut T, self.len);
                }
                p
            };

            let ptr = NonNull::new(new_ptr as *mut T).unwrap_or_else(|| alloc::handle_alloc_error(new_layout));
            self.data.heap = ptr;
            self.cap = new_cap;
        }
    }

    /// Move heap contents back inline if they fit, or shrink the allocation to `len`.
    pub fn shrink_to_fit(&mut self) {
        if !self.is_spilled() {
            return;
        }

        unsafe {
            let old_cap = self.cap;
            let old_layout = Layout::array::<T>(old_cap).unwrap_unchecked();
            let heap_ptr = self.data.heap.as_ptr();

            if self.len <= N {
                self.data.inline = ManuallyDrop::new([const { MaybeUninit::uninit() }; N]);
                ptr::copy_nonoverlapping(heap_ptr, ptr::addr_of_mut!(self.data.inline) as *mut T, self.len);
                alloc::dealloc(heap_ptr as *mut u8, old_layout);
                self.cap = N;
            } else if self.len < old_cap {
                let new_layout = Layout::array::<T>(self.len).unwrap_unchecked();
                let p = alloc::realloc(heap_ptr as *mut u8, old_layout, new_layout.size());
                if let Some(ptr) = NonNull::new(p as *mut T) {
                    self.data.heap = ptr;
                    self.cap = self.len;
                }
            }
        }
    }
}

impl<T, const N: usize> Drop for InlineVec<T, N> {
    fn drop(&mut self) {
        unsafe {
            let spilled = self.is_spilled();
            let cap = self.cap;
            ptr::drop_in_place(self.as_mut_slice() as *mut [T]);
            if spilled {
                let layout = Layout::array::<T>(cap).unwrap_unchecked();
                alloc::dealloc(self.data.heap.as_ptr() as *mut u8, layout);
            }
        }
    }
}

impl<T, const N: usize> Default for InlineVec<T, N> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Deref for InlineVec<T, N> {
    type Target = [T];
    #[inline(always)]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> DerefMut for InlineVec<T, N> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T: Clone, const N: usize> Clone for InlineVec<T, N> {
    fn clone(&self) -> Self {
        let mut v = Self::with_capacity(self.len);
        for item in self.as_slice() {
            v.push(item.clone());
        }
        v
    }
}

impl<T: PartialEq<U>, U, const N: usize, const M: usize> PartialEq<InlineVec<U, M>> for InlineVec<T, N> {
    #[inline]
    fn eq(&self, other: &InlineVec<U, M>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq, const N: usize> Eq for InlineVec<T, N> {}

impl<T: PartialOrd, const N: usize> PartialOrd for InlineVec<T, N> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T: Ord, const N: usize> Ord for InlineVec<T, N> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T: std::hash::Hash, const N: usize> std::hash::Hash for InlineVec<T, N> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

impl<T: std::fmt::Debug, const N: usize> std::fmt::Debug for InlineVec<T, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_slice(), f)
    }
}

impl<T, const N: usize> AsRef<[T]> for InlineVec<T, N> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self
    }
}

impl<T, const N: usize> AsMut<[T]> for InlineVec<T, N> {
    #[inline]
    fn as_mut(&mut self) -> &mut [T] {
        self
    }
}

impl<T, const N: usize, I: std::slice::SliceIndex<[T]>> std::ops::Index<I> for InlineVec<T, N> {
    type Output = I::Output;
    #[inline(always)]
    fn index(&self, i: I) -> &Self::Output {
        std::ops::Index::index(self.as_slice(), i)
    }
}

impl<T, const N: usize, I: std::slice::SliceIndex<[T]>> std::ops::IndexMut<I> for InlineVec<T, N> {
    #[inline(always)]
    fn index_mut(&mut self, i: I) -> &mut Self::Output {
        std::ops::IndexMut::index_mut(self.as_mut_slice(), i)
    }
}

impl<T, const N: usize> Extend<T> for InlineVec<T, N> {
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let it = iter.into_iter();
        self.reserve(it.size_hint().0);
        for item in it {
            self.push(item);
        }
    }
}

impl<T, const N: usize> FromIterator<T> for InlineVec<T, N> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut v = Self::new();
        v.extend(iter);
        v
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a InlineVec<T, N> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut InlineVec<T, N> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}

pub struct IntoIter<T, const N: usize> {
    vec: InlineVec<T, N>,
    head: usize,
    tail: usize,
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<T> {
        if self.head == self.tail {
            None
        } else {
            unsafe {
                let v = ptr::read(self.vec.as_ptr().add(self.head));
                self.head += 1;
                Some(v)
            }
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.tail - self.head;
        (n, Some(n))
    }
}

impl<T, const N: usize> DoubleEndedIterator for IntoIter<T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        if self.head == self.tail {
            None
        } else {
            unsafe {
                self.tail -= 1;
                Some(ptr::read(self.vec.as_ptr().add(self.tail)))
            }
        }
    }
}

impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {}

impl<T, const N: usize> Drop for IntoIter<T, N> {
    fn drop(&mut self) {
        unsafe {
            let base = self.vec.as_mut_ptr();
            let remaining: *mut [T] = std::ptr::slice_from_raw_parts_mut(base.add(self.head), self.tail - self.head);
            // Neutralize the vec's element drop; its Drop still frees the buffer.
            self.vec.len = 0;
            ptr::drop_in_place(remaining);
        }
    }
}

impl<T, const N: usize> IntoIterator for InlineVec<T, N> {
    type Item = T;
    type IntoIter = IntoIter<T, N>;
    #[inline]
    fn into_iter(self) -> IntoIter<T, N> {
        let tail = self.len;
        IntoIter { vec: self, head: 0, tail }
    }
}

impl<T, const N: usize, const M: usize> From<[T; M]> for InlineVec<T, N> {
    #[inline]
    fn from(arr: [T; M]) -> Self {
        let mut v = Self::with_capacity(M);
        unsafe {
            ptr::copy_nonoverlapping(arr.as_ptr(), v.as_mut_ptr(), M);
            mem::forget(arr);
            v.set_len(M);
        }
        v
    }
}

impl<T: Clone, const N: usize> From<&[T]> for InlineVec<T, N> {
    #[inline]
    fn from(s: &[T]) -> Self {
        let mut v = Self::with_capacity(s.len());
        for item in s {
            v.push(item.clone());
        }
        v
    }
}

#[macro_export]
macro_rules! invec {
    [] => { $crate::InlineVec::new() };

    [$($x:expr),+ $(,)?] => {{
        let mut v = $crate::InlineVec::new();
        $(v.push($x);)+
        v
    }};
}
