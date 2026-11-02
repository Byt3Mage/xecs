use crate::{
    Id, InlineVec,
    data_structures::VecIdxU32,
    memory::{RawBlock, RowLayout},
};

type Ids = InlineVec<Id, 4>;

pub(super) struct ManyEdges {
    targets: Ids,
    payload: RawBlock,
    capacity: u32,
}

impl ManyEdges {
    pub(super) fn new(layout: RowLayout) -> Self {
        Self {
            targets: Ids::new(),
            payload: layout.new_block(),
            capacity: 0,
        }
    }

    #[inline(always)]
    pub(super) fn len(&self) -> u32 {
        self.targets.len() as u32
    }

    #[inline(always)]
    pub(super) fn targets(&self) -> &[Id] {
        &self.targets
    }

    #[inline(always)]
    pub(super) fn payload(&self) -> &RawBlock {
        &self.payload
    }

    #[inline]
    pub(super) fn position(&self, target: Id) -> Option<u32> {
        self.targets.iter().position(|&t| t == target).map(|i| i as u32)
    }

    #[inline]
    pub(super) fn contains(&self, target: Id) -> bool {
        self.targets.contains(&target)
    }

    /// # Safety
    /// `T` is the declared payload type.
    pub(super) unsafe fn push<T: Copy>(&mut self, meta: RowLayout, target: Id, value: T) {
        let old = self.capacity;
        let new = (old + 1).next_power_of_two().max(4);

        if new > old {
            unsafe { self.payload.grow(meta, old, new) };
            self.capacity = new;
        }

        // SAFETY: just reserved, and rows past `len` are never live.
        unsafe { self.payload.set(meta.size(), self.len(), value) };
        self.targets.push(target);
    }

    /// Remove the edge at `at`, dropping its value. The last edge takes
    /// its place in both arrays.
    ///
    /// Safe: dropping and moving need the layout, not the type.
    pub(super) fn swap_remove(&mut self, row_size: usize, at: u32) -> Id {
        let last = self.len() - 1;
        // SAFETY: `at` is live. The value moving onto it comes from
        // `last`, which the target `swap_remove` below retires.
        unsafe { self.payload.swap_remove(row_size, at, last) };
        self.targets.swap_remove(at as usize)
    }

    /// Drop every value, release the buffer, and yield the targets.
    pub(super) fn dispose(&mut self, layout: RowLayout) -> Ids {
        unsafe { self.payload.dealloc(layout, self.capacity) };
        std::mem::take(&mut self.targets)
    }

    /// # Safety
    /// `at < len` and `T` is the declared payload type.
    #[inline]
    pub(super) unsafe fn value<T: Copy>(&self, size: usize, at: u32) -> &T {
        unsafe { self.payload.get(size, at) }
    }

    /// # Safety
    /// `at < len` and `T` is the declared payload type.
    #[inline]
    pub(super) unsafe fn value_mut<T: Copy>(&mut self, size: usize, at: u32) -> &mut T {
        unsafe { self.payload.get_mut(size, at) }
    }
}

impl Drop for ManyEdges {
    fn drop(&mut self) {
        // A list cannot free itself: the layout lives on the index.
        // Every removal path calls `take` first, so reaching here with
        // a buffer still held is a leak.
        debug_assert!(self.capacity == 0, "Edges dropped without `take`: payload leaked",);
    }
}

/// `unique_source`: one target per source, so the slot *is* the edge
/// and values are dense and slot-parallel.
pub(super) struct OneEdges {
    targets: VecIdxU32<Id>,
    payload: RawBlock,
}

impl OneEdges {
    pub(super) fn new(layout: &RowLayout) -> Self {
        Self {
            targets: VecIdxU32::new(),
            payload: layout.new_block(),
        }
    }

    #[inline(always)]
    pub(super) fn len(&self) -> u32 {
        self.targets.len()
    }

    #[inline(always)]
    pub(super) fn cap(&self) -> u32 {
        self.targets.cap()
    }

    #[inline(always)]
    pub(super) fn target(&self, slot: u32) -> Id {
        self.targets[slot]
    }

    pub(super) fn targets(&self) -> &[Id] {
        &self.targets
    }

    /// # Safety
    /// `T` is the declared payload type.
    pub(super) unsafe fn push<T: Copy>(&mut self, layout: RowLayout, target: Id, value: T) {
        let old = self.cap();
        self.targets.reserve(1);
        let new = self.cap();

        if new > old {
            unsafe { self.payload.grow(layout, old, new) };
        }

        // SAFETY: just reserved; rows past `len` are never live.
        unsafe { self.payload.set(layout.size(), self.len(), value) };
        self.targets.push(target);
    }

    /// Retarget a slot, replacing its value.
    ///
    /// # Safety
    /// `slot < len` and `T` is the declared payload type.
    pub(super) unsafe fn replace<T: Copy>(&mut self, row_size: usize, slot: u32, target: Id, value: T) {
        self.targets[slot] = target;
        // SAFETY: the slot is occupied, so its row is live.
        unsafe { self.payload.set(row_size, slot, value) }
    }

    pub(super) fn swap_remove(&mut self, row_size: usize, slot: u32) -> Id {
        let last = self.len() - 1;
        // SAFETY: `slot` is live. The value moving onto it comes from
        // `last`, which the target `swap_remove` below retires.
        unsafe { self.payload.shift(row_size, last, slot, 1) };
        self.targets.swap_remove(slot)
    }

    /// # Safety
    /// `slot < len` and `T` is the declared payload type.
    #[inline]
    pub(super) unsafe fn value<T: Copy>(&self, size: usize, slot: u32) -> &T {
        unsafe { self.payload.get(size, slot) }
    }

    /// # Safety
    /// `slot < len` and `T` is the declared payload type.
    #[inline]
    pub(super) unsafe fn value_mut<T: Copy>(&mut self, size: usize, slot: u32) -> &mut T {
        unsafe { self.payload.get_mut(size, slot) }
    }

    pub(super) fn dispose(&mut self, meta: RowLayout) {
        // SAFETY: one live row per slot.
        unsafe { self.payload.dealloc(meta, self.cap()) };
        self.targets.clear();
    }
}
