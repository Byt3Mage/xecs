use std::alloc::Layout;

use crate::{
    Id, TypeMeta,
    data_structures::{Sparse, VecIdxU32},
    inline_vec::InlineVec,
    invec,
    relation::{
        directed::Directed,
        hierarchy::{ChildIter, Hierarchy},
        symmetry::Symmetry,
    },
    storage::block::Block,
    type_meta::DropFn,
};

type Ids = InlineVec<Id, 4>;

const NONE: u32 = u32::MAX;

/// How edges relate their two ends.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Directed with a derived reverse index: both directions queryable.
    Directed {
        /// An [Id] appears as a source at most once in the relationship's edges.
        unique_source: bool,
        /// An [Id] appears as a target at most once in the relationship's edges.
        unique_target: bool,
        /// Rejects cyclical relationships.
        acyclic: bool,
        /// Creates a reverse index for (target, source) lookups.
        reverse: bool,
    },

    /// Endpoints are equivalent, which means acyclicity
    /// is unrepresentable ({a, b} is a 2-cycle).
    Symmetric { unique: bool },

    /// Directed with a tree index.
    Hierarchical,
}

impl Default for Shape {
    /// Default topology is a many to many non-acyclic relationship
    /// with no reverse index.
    fn default() -> Self {
        Shape::Directed {
            unique_source: false,
            unique_target: false,
            acyclic: false,
            reverse: false,
        }
    }
}

impl Shape {
    #[inline]
    pub fn is_symmetric(&self) -> bool {
        matches!(self, Self::Symmetric { .. })
    }
}

pub(super) struct Rows {
    data: Block,
    cap: u32,
}

impl Rows {
    pub(super) fn new(layout: Layout, drop: DropFn) -> Self {
        Self { data: Block::new(layout, drop), cap: 0 }
    }

    #[inline]
    pub(super) fn is_zst(&self) -> bool {
        self.data.is_zst()
    }

    pub(super) fn reserve(&mut self, need: u32) {
        if need <= self.cap {
            return;
        }
        let new = need.next_power_of_two().max(4);
        // SAFETY: new > cap, and cap is this block's current capacity.
        unsafe { self.data.realloc(self.cap, new) };
        self.cap = new;
    }

    /// # Safety
    /// `row` is within capacity and dead; `T` is the declared type.
    pub(super) unsafe fn write<T>(&mut self, row: u32, value: T) {
        if !self.is_zst() {
            unsafe { self.data.row_ptr(row).cast().write(value) };
        }
    }

    /// Place `value` inside `row` and drop the previously held value.
    ///
    /// # Safety
    /// `row` is within capacity and live; `T` is the declared type.
    pub(super) unsafe fn replace<T>(&mut self, row: u32, value: T) {
        if !self.is_zst() {
            let _ = unsafe { self.data.row_ptr(row).cast().replace(value) };
        }
    }

    /// Relocate `count` rows within this block. Ranges may overlap.
    ///
    /// # Safety
    /// Both ranges must lie within capacity. The values are *moved*:
    /// the source rows are dead afterwards and must not be dropped,
    /// and the destination must have held no live value.
    pub(super) unsafe fn move_within(&mut self, src: u32, dst: u32, count: u32) {
        if count == 0 || self.is_zst() {
            return;
        }
        unsafe {
            let len = count as usize * self.data.stride();
            self.data.row_ptr(src).copy_to(self.data.row_ptr(dst), len);
        }
    }

    /// Relocate `count` rows into another block.
    ///
    /// # Safety
    /// Both blocks store the same type; both ranges are within their
    /// capacities; the blocks are distinct allocations. Values are
    /// moved, with the same obligations as `shift`.
    pub(super) unsafe fn move_to(&self, src: u32, dest: &mut Rows, dst: u32, count: u32) {
        if count == 0 || self.is_zst() {
            return;
        }
        debug_assert_eq!(self.data.stride(), dest.data.stride());
        unsafe {
            let len = count as usize * self.data.stride();
            let src = self.data.row_ptr(src);
            let dst = dest.data.row_ptr(dst);
            src.copy_to_nonoverlapping(dst, len);
        }
    }

    /// # Safety
    /// `row` is within capacity and holds a live value.
    #[inline]
    pub(super) unsafe fn drop_row(&self, row: u32) {
        unsafe { self.data.drop_row(row) };
    }

    /// # Safety
    /// Every row in `range` is within capacity and holds a live value.
    pub(super) unsafe fn drop_range(&self, range: std::ops::Range<u32>) {
        unsafe { range.for_each(|i| self.data.drop_row(i)) };
    }

    /// Drop the value at `row` and move the last one into its place,
    /// mirroring `Vec::swap_remove` for a caller whose length lives
    /// elsewhere.
    ///
    /// `last` is the index of the final live row *before* the removal.
    /// The drop happens first: the row must be dead before anything is
    /// moved onto it, or the incoming value would be destroyed instead.
    ///
    /// # Safety
    /// - `row <= last`, and both are within capacity.
    /// - Every row in `0..=last` is live.
    /// - The caller's own length becomes `last` afterwards, so the row
    ///   at `last` is dead on return and must not be dropped again.
    #[inline]
    pub(super) unsafe fn swap_remove(&mut self, row: u32, last: u32) {
        debug_assert!(row <= last);
        unsafe {
            self.drop_row(row);
            if row != last {
                let src = self.data.row_ptr(last);
                let dst = self.data.row_ptr(row);
                src.copy_to_nonoverlapping(dst, self.data.stride());
            }
        }
    }

    /// Drop the live rows and release the allocation.
    ///
    /// # Safety
    /// `live` is the range holding live values, per the payload
    /// invariant.
    pub(super) unsafe fn dispose(&mut self, live: std::ops::Range<u32>) {
        unsafe {
            // len == 0: all rows are dropped,
            // so data only releases the allocation.
            live.for_each(|i| self.data.drop_row(i));
            self.dealloc();
        }
    }

    /// Release the allocation. Live rows must already be dropped.
    ///
    /// # Safety
    /// No row in this block is live.
    pub(super) unsafe fn dealloc(&mut self) {
        // len 0: nothing is dropped, the allocation is released.
        unsafe { self.data.drop(0, self.cap) };
    }
}

pub enum Target {
    One { targets: VecIdxU32<Id>, data: Rows },
    Many { targets: VecIdxU32<Ids> },
}

/// A read of one entity's edges in one direction.
/// `One` yields at most one id. `Many` yields a slice.
/// 'Children` yields the subtree of the hierarchy.
pub(crate) enum Edges<'a> {
    One(Id),
    Many(&'a [Id]),
    Children(ChildIter<'a>),
}

impl<'a> Edges<'a> {
    pub fn into_iter(self) -> EdgeIter<'a> {
        match self {
            Edges::One(id) => EdgeIter::One(Some(id)),
            Edges::Many(ids) => EdgeIter::Many(ids.iter()),
            Edges::Children(c) => EdgeIter::Children(c),
        }
    }
}

pub(crate) enum EdgeIter<'a> {
    One(Option<Id>),
    Many(std::slice::Iter<'a, Id>),
    Children(ChildIter<'a>),
}
impl<'a> EdgeIter<'a> {
    #[inline]
    pub const fn empty() -> Self {
        Self::One(None)
    }
}

impl Iterator for EdgeIter<'_> {
    type Item = Id;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EdgeIter::One(v) => v.take(),
            EdgeIter::Many(it) => it.next().copied(),
            EdgeIter::Children(c) => c.next(),
        }
    }
}

pub(crate) struct Index {
    pub(crate) sparse: Sparse<u32>,
    pub(crate) source: VecIdxU32<Id>,
    pub(crate) target: Target,
}

impl Index {
    pub(crate) fn new(unique: bool, meta: &TypeMeta) -> Self {
        Self {
            sparse: Sparse::new(),
            source: VecIdxU32::new(),
            target: match unique {
                true => Target::One {
                    targets: VecIdxU32::new(),
                    data: Rows::new(meta.layout, meta.dtor),
                },
                false => Target::Many { targets: VecIdxU32::new() },
            },
        }
    }
}

impl Index {
    #[inline(always)]
    pub(crate) fn is_empty(&self, id: Id) -> bool {
        !self.sparse.contains(id)
    }

    /// One id's edges in this direction.
    #[inline(always)]
    pub(crate) fn get(&self, id: Id) -> Option<Edges<'_>> {
        self.sparse.get(id).map(|&s| match &self.target {
            Target::One { targets, .. } => Edges::One(targets[s]),
            Target::Many { targets, .. } => Edges::Many(&targets[s]),
        })
    }

    /// One id's edges in this direction.
    #[inline(always)]
    pub(crate) fn get_unique(&self, id: Id) -> Option<Id> {
        self.sparse.get(id).and_then(|&s| match &self.target {
            Target::One { targets, .. } => Some(targets[s]),
            Target::Many { .. } => None,
        })
    }

    #[inline(always)]
    pub(crate) fn contains(&self, source: Id, target: Id) -> bool {
        self.sparse.get(source).is_some_and(|&s| match &self.target {
            Target::One { targets, .. } => targets[s] == target,
            Target::Many { targets, .. } => targets[s].contains(&target),
        })
    }

    /// Insert `(source, target)`.
    ///
    /// Returns false and drops `payload` with the existing value
    /// if the pair already exists.
    ///
    /// # Safety
    /// `T` is the relation's declared payload type.
    pub(super) unsafe fn add<T>(&mut self, source: Id, target: Id, payload: T) -> bool {
        if let Some(&slot) = self.sparse.get(source) {
            return match &mut self.target {
                Target::One { targets, data } => {
                    if targets[slot] == target {
                        return false;
                    }
                    // Replace current edge with new id and payload
                    targets[slot] = target;
                    unsafe { data.replace(slot, payload) };
                    true
                }
                Target::Many { targets } => {
                    if targets[slot].contains(&target) {
                        // todo: update existing payload
                        return false;
                    }

                    targets[slot].push(target);
                    // todo: add new payload data
                    true
                }
            };
        }

        let slot = self.source.len();
        self.source.push(source);

        match &mut self.target {
            Target::One { targets, data } => {
                targets.push(target);
                data.reserve(slot + 1);
                // SAFETY: reserved above and never written.
                unsafe { data.write(slot, payload) };
            }
            Target::Many { targets } => {
                targets.push(Ids::from([target]));
                // todo: write new payload data
            }
        }
        self.sparse.set(source, slot);
        true
    }

    pub(crate) fn remove(&mut self, source: Id, target: Id) -> bool {
        let Some(&slot) = self.sparse.get(source) else { return false };

        match &mut self.target {
            Target::One { targets, data } => {
                if targets[slot] != target {
                    return false;
                }

                self.sparse.remove(source);
                self.source.swap_remove(slot);
                targets.swap_remove(slot);
                unsafe { data.swap_remove(slot, self.source.len()) };

                if slot < self.source.len() {
                    self.sparse.set(self.source[slot], slot);
                }
            }
            Target::Many { targets } => {
                let list = &mut targets[slot];
                let Some(at) = list.iter().position(|&t| t == target) else { return false };
                list.swap_remove(at);

                if list.is_empty() {
                    self.sparse.remove(source);
                    self.source.swap_remove(slot);
                    targets.swap_remove(slot);

                    if slot < self.source.len() {
                        self.sparse.set(self.source[slot], slot);
                    }
                }
            }
        }

        true
    }

    /// Remove all edges of `source`, returning all its targets.
    pub(crate) fn remove_all(&mut self, source: Id) -> Ids {
        let Some(&slot) = self.sparse.get(source) else { return invec![] };

        self.sparse.remove(source);
        self.source.swap_remove(slot);

        let ids = match &mut self.target {
            Target::One { targets, data } => {
                let id = targets.swap_remove(slot);
                unsafe { data.swap_remove(slot, self.source.len()) };
                std::iter::once(id).collect()
            }
            Target::Many { targets } => {
                let list = targets.swap_remove(slot);
                list
            }
        };

        if slot < self.source.len() {
            self.sparse.set(self.source[slot], slot);
        }

        ids
    }

    /// The value on `(source, target)`.
    ///
    /// # Safety
    /// `T` is the relation's declared payload type.
    pub(crate) unsafe fn payload<T>(&self, source: Id, target: Id) -> Option<&T> {
        let &slot = self.sparse.get(source)?;
        match &self.target {
            Target::One { targets, data } => {
                (targets[slot] == target).then(|| unsafe { data.data.row_ptr(slot).cast().as_ref() })
            }
            Target::Many { targets } => {
                let at = targets[slot].iter().position(|&t| t == target)?;
                todo!("get data");
            }
        }
    }

    /// The value on `(source, target)`.
    ///
    /// # Safety
    /// `T` is the relation's declared payload type.
    pub(crate) unsafe fn payload_mut<T>(&mut self, source: Id, target: Id) -> Option<&mut T> {
        let &slot = self.sparse.get(source)?;
        match &self.target {
            Target::One { targets, data } => {
                (targets[slot] == target).then(|| unsafe { data.data.row_ptr(slot).cast().as_mut() })
            }
            Target::Many { targets } => {
                let at = targets[slot].iter().position(|&t| t == target)?;
                todo!("get data");
            }
        }
    }

    /// Collect every key whose values include `value`.
    ///
    /// A linear scan of the dense arrays. This is the cost of a
    /// directed relation declared without a reverse index, where the
    /// answer would otherwise be one lookup. Only `purge` reaches this;
    /// unpinned reversed queries are rejected at plan-build time rather
    /// than falling back to a scan.
    ///
    /// Appends to `out` rather than returning, so callers can hold the
    /// buffer as scratch and never allocate.
    pub(crate) fn find_sources(&self, target: Id, out: &mut Vec<Id>) {
        match &self.target {
            Target::One { targets, .. } => out.extend(
                self.source
                    .iter()
                    .zip(targets.iter())
                    .filter_map(|(&k, &v)| (v == target).then_some(k)),
            ),
            Target::Many { targets, .. } => out.extend(
                self.source
                    .iter()
                    .zip(targets.iter())
                    .filter_map(|(&k, vs)| vs.contains(&target).then_some(k)),
            ),
        }
    }
}

pub(crate) enum Storage {
    Directed(Directed),
    Symmetry(Symmetry),
    Hierarchy(Hierarchy),
}

impl Storage {
    pub fn select(shape: Shape, meta: &TypeMeta) -> Self {
        match shape {
            Shape::Hierarchical => Storage::Hierarchy(Hierarchy::new(meta)),
            Shape::Symmetric { unique } => Storage::Symmetry(Symmetry::new(unique, meta)),
            Shape::Directed { unique_source, unique_target, acyclic, reverse } => {
                Storage::Directed(Directed::new(unique_source, unique_target, acyclic, reverse, meta))
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RelateError {
    #[error("{0} forms self-edge on a symmetric relation")]
    SelfEdgeOnSymmetric(Id),

    #[error("{0} forms self-edge on a hierarchical relation")]
    SelfEdgeOnHierarchy(Id),

    #[error("{0} -> {1} closes a loop on an acyclic relation")]
    CycleOnAcyclic(Id, Id),
}
