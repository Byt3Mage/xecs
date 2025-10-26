pub trait SparseIndex: PartialEq + Clone + Eq {
    fn to_sparse_index(&self) -> usize;
}

impl SparseIndex for usize {
    #[inline(always)]
    fn to_sparse_index(&self) -> usize {
        *self
    }
}

pub(crate) struct Entry<K: SparseIndex, V = K> {
    pub(crate) key: K,
    pub(crate) value: V,
}

pub struct SparseSet<K: SparseIndex + PartialEq, V> {
    dense: Vec<Entry<K, V>>,
    sparse: Vec<usize>,
}

impl<K: SparseIndex + PartialEq, V> SparseSet<K, V> {
    const INVALID_DENSE_IDX: usize = usize::MAX;

    pub fn new() -> Self {
        Self {
            dense: vec![],
            sparse: vec![],
        }
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data and returns the old value if the entry is already in the set.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let sparse_idx = key.to_sparse_index();

        if sparse_idx >= self.sparse.len() {
            self.sparse.resize(sparse_idx + 1, Self::INVALID_DENSE_IDX);
        }

        // SAFETY: bounds checked and resized above.
        let dense_idx = unsafe { self.sparse.get_unchecked_mut(sparse_idx) };

        match self.dense.get_mut(*dense_idx) {
            Some(entry) =>  Some(std::mem::replace(&mut entry.value, value)),
            None => {
                *dense_idx = self.dense.len();
                self.dense.push(Entry {key, value});
                None
            }
        }
    }

    /// Removes an entry from the set.
    /// Returns the value associated with the key if it was present.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let sparse = key.to_sparse_index();
        let dense_idx = self.sparse.get_mut(sparse)?;

        if *dense_idx >= self.dense.len() {
            return None;
        }

        let dense_idx = std::mem::replace(dense_idx, Self::INVALID_DENSE_IDX);
        let removed = self.dense.swap_remove(dense_idx).value;

        if let Some(entry) = self.dense.get(dense_idx) {
            self.sparse[entry.key.to_sparse_index()] = dense_idx;
        }

        Some(removed)
    }

    #[inline]
    pub fn contains_key(&self, key: &K) -> bool {
        match self.sparse.get(key.to_sparse_index()) {
            Some(&dense_idx) => dense_idx < self.dense.len(),
            None => false,
        }
    }

    #[inline]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.sparse
            .get(key.to_sparse_index())
            .and_then(|&dense_idx| self.dense.get(dense_idx))
            .map(|e| &e.value)
    }

    #[inline]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.sparse
            .get(key.to_sparse_index())
            .and_then(|&dense_idx| self.dense.get_mut(dense_idx))
            .map(|e| &mut e.value)
    }
}
