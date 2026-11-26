use core::slice;
use std::ptr::NonNull;

use crate::{
    HasMeta, Id,
    memory::{RawBlock, RowLayout},
    relation::edge::Edge,
};

/// One source's edges for a relation whose targets are Many: a
/// `Vec<Edge<T>>` whose `T` is known only through the edge layout.
///
/// Dropping a list does not free its allocation; `free` must be called
/// first with the edge layout.
///
/// Invariants:
/// - `ptr` holds `cap` edges; the first `len` are initialized.
/// - When `cap == 0` nothing is allocated, and `ptr` is dangling and aligned
///   for the edge layout.
pub(crate) struct Many {
    data: RawBlock,
    len: u32,
    cap: u32,
}

impl Many {
    /// An empty list for edges of layout `edge`. Does not allocate.
    pub(crate) fn new(edge: RowLayout) -> Self {
        Self { data: edge.new_block(), len: 0, cap: 0 }
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> u32 {
        self.len
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The target of edge `i`.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    #[inline]
    pub(crate) fn target(&self, stride: usize, i: u32) -> Id {
        assert!(i < self.len, "edge index {i} out of bounds");
        // SAFETY: edge `i` is initialized, and its target sits at offset 0,
        // aligned for `Id`.
        unsafe { self.row(stride, i).cast::<Id>().read() }
    }

    /// The targets of all edges, in list order.
    #[inline]
    pub(crate) fn targets(&self, stride: usize) -> impl Iterator<Item = Id> + '_ {
        // SAFETY: every `i < len` is an initialized edge with its target at offset 0.
        (0..self.len).map(move |i| unsafe { self.row(stride, i).cast::<Id>().read() })
    }

    /// The position of the edge to `target`, if any.
    #[inline]
    pub(crate) fn position(&self, stride: usize, tgt: Id) -> Option<u32> {
        // SAFETY: as in `targets`.
        (0..self.len).find(|&i| unsafe { self.row(stride, i).cast::<Id>().read() } == tgt)
    }

    /// Edge `i`'s bytes.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    #[inline]
    pub(crate) fn get_ptr(&self, stride: usize, i: u32) -> NonNull<u8> {
        assert!((i < self.len), "edge index {i} out of bounds");
        self.row(stride, i)
    }

    /// Appends the edge at `src`. Returns its position.
    ///
    /// # Safety
    /// `edge` is this list's layout; `src` is valid for reads of one edge
    /// and does not point into this list.
    pub(crate) unsafe fn push_raw(&mut self, edge: RowLayout, src: NonNull<u8>) {
        if self.len == self.cap {
            self.grow(edge);
        }
        let stride = edge.size();
        // SAFETY: `i < cap` after growing; the caller guarantees `src`.
        unsafe { self.row(stride, self.len).copy_from_nonoverlapping(src, stride) };
        self.len += 1;
    }

    /// Removes every edge. The allocation is kept.
    ///
    /// Since this list is built from copy-only types,
    /// we simply set the length to 0.
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.len = 0;
    }

    /// Frees the allocation, leaving an empty list.
    ///
    /// # Safety
    /// `edge` is this list's layout.
    pub(crate) unsafe fn free(&mut self, edge: RowLayout) {
        // SAFETY: `ptr` was allocated with `array(edge, cap)`
        unsafe { self.data.dealloc(edge, self.cap) };
        self.len = 0;
        self.cap = 0;
    }

    /// All edges, in list order.
    ///
    /// # Safety
    /// This list's layout is that of `Edge<T>`.
    #[inline]
    pub(crate) unsafe fn as_slice<T: HasMeta>(&self) -> &[Edge<T>] {
        // SAFETY: `len` initialized `Edge<T>`s at an address aligned for them.
        unsafe { slice::from_raw_parts(self.data.ptr().cast::<Edge<T>>().as_ptr(), self.len as usize) }
    }

    /// All edges, in list order. Targets must not be changed.
    ///
    /// # Safety
    /// This list's layout is that of `Edge<T>`.
    #[inline]
    pub(crate) unsafe fn as_mut_slice<T: HasMeta>(&mut self) -> &mut [Edge<T>] {
        // SAFETY: as in `as_slice`, and `&mut self` makes the borrow unique.
        unsafe { slice::from_raw_parts_mut(self.data.ptr().cast::<Edge<T>>().as_ptr(), self.len as usize) }
    }

    /// # Safety
    /// This list's layout is that of `Edge<T>`.
    #[inline]
    pub(crate) unsafe fn get<T: HasMeta>(&self, i: u32) -> Option<&Edge<T>> {
        unsafe { self.as_slice::<T>() }.get(i as usize)
    }

    /// # Safety
    /// This list's layout is that of `Edge<T>`.
    #[inline]
    pub(crate) unsafe fn get_mut<T: HasMeta>(&mut self, i: u32) -> Option<&mut Edge<T>> {
        unsafe { self.as_mut_slice::<T>() }.get_mut(i as usize)
    }

    /// The edge to `target`, if any.
    ///
    /// # Safety
    /// This list's layout is that of `Edge<T>`.
    #[inline]
    pub(crate) unsafe fn find_mut<T: HasMeta>(&mut self, target: Id) -> Option<&mut Edge<T>> {
        unsafe { self.as_mut_slice::<T>() }
            .iter_mut()
            .find(|e| e.target == target)
    }

    /// Appends `edge`. Returns its position.
    ///
    /// # Safety
    /// This list's layout is that of `Edge<T>`.
    pub(crate) unsafe fn push<T: HasMeta>(&mut self, edge: Edge<T>) {
        let layout = RowLayout::of::<Edge<T>>();
        if self.len == self.cap {
            self.grow(layout);
        }
        // SAFETY: `i < cap` after growing, at an address aligned for `Edge<T>`.
        unsafe { self.row(layout.size(), self.len).cast::<Edge<T>>().write(edge) };
        self.len += 1;
    }

    /// The address of edge `i`, for `i <= cap`.
    #[inline(always)]
    fn row(&self, stride: usize, i: u32) -> NonNull<u8> {
        // SAFETY: `i <= cap`, so the offset stays within or one past the allocation.
        unsafe { self.data.row(stride, i) }
    }

    /// Removes and returns edge `i`, moving the last edge into its place.
    ///
    /// # Safety
    /// This list's layout is that of `Edge<T>`.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    pub(crate) unsafe fn swap_remove_as<T: HasMeta>(&mut self, i: u32) -> Edge<T> {
        assert!((i < self.len), "edge index {i} out of bounds");
        let stride = size_of::<Edge<T>>();
        // SAFETY: edge `i` is an initialized `Edge<T>`.
        let removed = unsafe { self.row(stride, i).cast::<Edge<T>>().read() };
        self.swap_remove(stride, i);
        removed
    }

    /// Removes edge `i`, moving the last edge into its place.
    ///
    /// # Panics
    /// Panics if `i >= len`.
    pub(crate) fn swap_remove(&mut self, stride: usize, i: u32) {
        assert!(i < self.len, "edge index {i} out of bounds");
        let last = self.len - 1;
        // SAFETY: `i` and `last` are distinct initialized edges.
        unsafe { self.data.swap_remove(stride, i, last) };
        self.len = last;
    }

    /// Doubles the capacity, starting at 4.
    fn grow(&mut self, edge: RowLayout) {
        let new_cap = if self.cap == 0 { 4 } else { self.cap.checked_mul(2).expect("capacity overflow") };
        // SAFETY: self.cap is current capacity and new_cap > self.cap
        unsafe { self.data.grow(edge, self.cap, new_cap) };
        self.cap = new_cap;
    }
}
