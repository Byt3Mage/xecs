use std::{
    alloc::Layout,
    ops::{Index, IndexMut},
    ptr::{self},
    usize,
};

pub trait SparseIndex: PartialEq + Clone + Eq {
    fn to_sparse_index(&self) -> usize;
}

impl SparseIndex for usize {
    #[inline(always)]
    fn to_sparse_index(&self) -> usize {
        *self
    }
}

struct Page {
    dense_indices: Box<[usize]>,
    size: usize,
    is_active: bool,
}

impl Index<usize> for Page {
    type Output = usize;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.dense_indices[index]
    }
}

impl IndexMut<usize> for Page {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.dense_indices[index]
    }
}

impl Page {
    fn new(size: usize) -> Self {
        assert!(size != 0, "can't create empty page");

        Self {
            dense_indices: Box::from([]),
            size,
            is_active: false,
        }
    }

    fn set_active(&mut self) -> &mut Self {
        if self.is_active {
            return self;
        }

        assert!(self.size != 0, "can't create empty page");

        self.dense_indices = unsafe {
            let layout = Layout::array::<usize>(self.size).unwrap();
            let ptr = std::alloc::alloc(layout) as *mut usize;

            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            // SAFETY: The pointer is valid and aligned, and the layout is correct.
            ptr::write_bytes(ptr, 0xFF, self.size);
            Box::from_raw(std::slice::from_raw_parts_mut(ptr, self.size))
        };
        self.is_active = true;
        self
    }
}

pub(crate) struct Entry<K: SparseIndex, V = K> {
    key: K,
    pub(crate) value: V,
}

impl<K: SparseIndex, V> Entry<K, V> {
    #[inline]
    pub fn key(&self) -> &K {
        &self.key
    }
}

/// Specialized sparse set of entities with associated data.
pub(crate) struct PagedSparseSet<K: SparseIndex, V> {
    dense: Vec<Entry<K, V>>,
    pages: Vec<Page>,
    page_size: usize,
    page_bits: usize,
    page_mask: usize,
}

impl<K: SparseIndex, V> PagedSparseSet<K, V> {
    pub(crate) fn new(page_size: usize) -> Self {
        Self {
            dense: vec![],
            pages: vec![],
            page_size,
            page_bits: page_size.trailing_zeros() as usize,
            page_mask: page_size - 1,
        }
    }

    fn ensure_page(pages: &mut Vec<Page>, page_index: usize, page_size: usize) -> &mut Page {
        if page_index >= pages.len() {
            pages.resize_with(page_index + 1, || Page::new(page_size));
        }
        pages[page_index].set_active()
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data if the entity is already in the set.
    pub(crate) fn insert(&mut self, key: K, value: V) {
        let sparse = key.to_sparse_index();
        let page = Self::ensure_page(&mut self.pages, sparse >> self.page_bits, self.page_size);
        let page_offset = sparse & self.page_mask;
        let dense = page[page_offset];

        if dense != usize::MAX {
            debug_assert!(dense < self.dense.len(), "dense index out of bounds");
            self.dense[dense] = Entry { key, value };
        } else {
            page[page_offset] = self.dense.len();
            self.dense.push(Entry { key, value });
        }
    }

    /// Removes an entity from the set.
    /// Returns the value associated with the entity if it was present.
    pub(crate) fn remove(&mut self, key: &K) -> Option<Entry<K, V>> {
        let sparse = key.to_sparse_index();
        let page = match self.pages.get_mut(sparse >> self.page_bits) {
            Some(page) if page.is_active => page,
            _ => return None,
        };

        let page_offset = sparse & self.page_mask;
        let dense = page[page_offset];

        // key not in set.
        if dense == usize::MAX {
            return None;
        }

        page[page_offset] = usize::MAX;

        let removed = self.dense.swap_remove(dense);

        if dense != self.dense.len() {
            let sparse_index = self.dense[dense].key.to_sparse_index();
            let page = &mut self.pages[sparse_index >> self.page_bits];

            assert!(page.is_active);

            page[sparse_index & self.page_mask] = dense;
        }

        Some(removed)
    }

    pub(crate) fn contains(&self, key: &K) -> bool {
        let sparse = key.to_sparse_index();
        match self.pages.get(sparse >> self.page_bits) {
            Some(page) => page.is_active && page[sparse & self.page_mask] < self.dense.len(),
            None => false,
        }
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        let sparse = key.to_sparse_index();
        match self.pages.get(sparse >> self.page_bits) {
            Some(page) if page.is_active => {
                let dense = page[sparse & self.page_mask];

                if dense < self.dense.len() {
                    Some(&self.dense[dense].value)
                } else {
                    None
                }
            }
            _ => return None,
        }
    }

    pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let sparse = key.to_sparse_index();
        match self.pages.get(sparse >> self.page_bits) {
            Some(page) if page.is_active => {
                let dense = page[sparse & self.page_mask];

                if dense < self.dense.len() {
                    Some(&mut self.dense[dense].value)
                } else {
                    None
                }
            }
            _ => return None,
        }
    }
}

pub struct SparseSet<K: SparseIndex, V> {
    dense: Vec<Entry<K, V>>,
    sparse: Vec<usize>,
}

impl<K: SparseIndex, V> SparseSet<K, V> {
    pub fn new() -> Self {
        Self {
            dense: vec![],
            sparse: vec![],
        }
    }

    /// Resizes the sparse array such that
    /// it can hold at least (`index` + 1) entries.
    #[inline(always)]
    fn ensure_index(&mut self, index: usize) -> usize {
        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, usize::MAX);
        }

        self.sparse[index]
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data if the entity is already in the set.
    pub fn insert(&mut self, key: K, value: V) {
        let sparse = key.to_sparse_index();
        let dense = self.ensure_index(sparse);

        if dense != usize::MAX {
            debug_assert!(dense < self.dense.len(), "dense index is out of bounds");
            self.dense[dense] = Entry { key, value };
        } else {
            self.sparse[sparse] = self.dense.len();
            self.dense.push(Entry { key, value });
        }
    }

    /// Removes an entity from the set.
    /// Returns the value associated with the entity if it was present.
    pub(crate) fn remove(&mut self, key: &K) -> Option<Entry<K, V>> {
        let sparse = key.to_sparse_index();
        let dense = *(self.sparse.get(sparse)?);

        // entity not in set.
        if dense == usize::MAX {
            return None;
        }

        debug_assert!(dense < self.dense.len(), "dense index is out of bounds");

        self.sparse[sparse] = usize::MAX;

        let removed = self.dense.swap_remove(dense);

        if dense != self.dense.len() {
            self.sparse[self.dense[dense].key.to_sparse_index()] = dense;
        }

        Some(removed)
    }

    #[inline]
    pub fn contains(&self, key: &K) -> bool {
        match self.sparse.get(key.to_sparse_index()) {
            Some(&dense) => dense < self.dense.len(),
            None => false,
        }
    }

    #[inline]
    pub fn get(&self, key: &K) -> Option<&V> {
        match self.sparse.get(key.to_sparse_index()) {
            Some(&dense) if dense < self.dense.len() => Some(&self.dense[dense].value),
            _ => None,
        }
    }

    #[inline]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        match self.sparse.get(key.to_sparse_index()) {
            Some(&dense) if dense < self.dense.len() => Some(&mut self.dense[dense].value),
            _ => None,
        }
    }
}
