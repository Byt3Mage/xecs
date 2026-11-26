use crate::{Id, utils::NonMaxU32};

const PAGE_BITS: usize = 10;
const PAGE_SIZE: usize = 1 << PAGE_BITS;
const PAGE_MASK: usize = (1 << PAGE_BITS) - 1;

const fn split(index: u32) -> (usize, usize) {
    let idx = index as usize;
    (idx >> PAGE_BITS, idx & PAGE_MASK)
}

#[inline(always)]
fn dense(n: usize) -> u32 {
    u32::try_from(n)
        .ok()
        .filter(|&d| d < u32::MAX)
        .expect("sparse set holds at most u32::MAX - 1 entries")
}

/// Dense positions for `PAGE_SIZE` consecutive entity indices. An entry holds
/// the dense position plus one.
type Page = Box<[Option<NonMaxU32>; PAGE_SIZE]>;

/// Dense positions by entity index, allocated one page at a time.
#[derive(Default)]
struct SparseIndex {
    pages: Vec<Option<Page>>,
}

impl SparseIndex {
    /// The dense position stored for `index`.
    #[inline]
    fn get(&self, index: u32) -> Option<u32> {
        let (page, slot) = split(index);
        Some(self.pages.get(page)?.as_ref()?[slot]?.get())
    }

    /// Stores `dense` for `index`, allocating its page if needed.
    /// `dense` is below `u32::MAX`.
    #[inline]
    fn set(&mut self, index: u32, dense: u32) {
        let (page, slot) = split(index);
        if page >= self.pages.len() {
            self.pages.resize_with(page + 1, || None);
        }
        let page = self.pages[page].get_or_insert_with(|| Box::new([None; PAGE_SIZE]));
        page[slot] = NonMaxU32::new(dense);
    }

    /// Removes the entry for `index`. Pages are kept.
    #[inline]
    fn clear(&mut self, index: u32) {
        let (page, slot) = split(index);
        if let Some(Some(page)) = self.pages.get_mut(page) {
            page[slot] = None;
        }
    }
}

struct Entry<T> {
    id: Id,
    value: T,
}

pub(crate) struct Sparse<T> {
    sparse: SparseIndex,
    values: Vec<Entry<T>>,
}

impl<T> Default for Sparse<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Sparse<T> {
    #[inline(always)]
    pub fn new() -> Self {
        Self { sparse: SparseIndex::default(), values: Vec::new() }
    }

    /// Moves the `value` into the entry for `id`. An existing entry
    /// for `id`'s index is overwritten, including one held by an older
    /// generation.
    pub fn insert(&mut self, id: Id, value: T) {
        if let Some(d) = self.sparse.get(id.index) {
            self.values[d as usize] = Entry { id, value };
            return;
        }
        let dense = dense(self.values.len());
        self.values.push(Entry { id, value });
        self.sparse.set(id.index, dense);
    }

    /// The value at `id`, inserting `f()` first when there is none.
    pub fn get_or_insert(&mut self, id: Id, f: impl FnOnce() -> T) -> &mut T {
        let dense = match self.sparse.get(id.index) {
            Some(d) => d as usize,
            None => {
                let d = self.values.len();
                let dense = dense(d);
                self.values.push(Entry { id, value: f() });
                self.sparse.set(id.index, dense);
                d
            }
        };

        &mut self.values[dense].value
    }

    /// Removes the entry for `id`, moving the last entry into its place.
    pub fn remove(&mut self, id: Id) {
        let Some(dense) = self.sparse.get(id.index) else { return };
        let d = dense as usize;
        let last = self.values.len() - 1;

        if d != last {
            self.sparse.set(self.values[d].id.index, dense);
        }

        self.values.swap_remove(d);
        self.sparse.clear(id.index);
    }

    pub fn get(&self, id: Id) -> Option<&T> {
        self.sparse.get(id.index).map(|d| &self.values[d as usize].value)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut T> {
        self.sparse.get(id.index).map(|d| &mut self.values[d as usize].value)
    }
}

impl<T> std::ops::Index<Id> for Sparse<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, i: Id) -> &T {
        self.get(i).unwrap()
    }
}

impl<T> std::ops::IndexMut<Id> for Sparse<T> {
    #[inline(always)]
    fn index_mut(&mut self, i: Id) -> &mut T {
        self.get_mut(i).unwrap()
    }
}
