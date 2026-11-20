use crate::Id;

pub const fn ix(id: Id) -> usize {
    id.index() as usize
}

pub struct Sparse<T> {
    values: Vec<Option<T>>,
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
        Self { values: vec![] }
    }

    pub fn get(&self, id: Id) -> Option<&T> {
        self.values.get(ix(id)).and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut T> {
        self.values.get_mut(ix(id)).and_then(Option::as_mut)
    }

    pub fn set(&mut self, id: Id, value: T) -> Option<T> {
        let idx = ix(id);
        if idx >= self.values.len() {
            self.values.resize_with(idx + 1, || None);
        }
        self.values[idx].replace(value)
    }

    /// The value at `id`, inserting `f()` first when there is none.
    pub fn get_or_insert_with(&mut self, id: Id, f: impl FnOnce() -> T) -> &mut T {
        let idx = ix(id);
        if idx >= self.values.len() {
            self.values.resize_with(idx + 1, || None);
        }
        self.values[idx].get_or_insert_with(f)
    }

    pub fn remove(&mut self, id: Id) -> Option<T> {
        self.values.get_mut(ix(id)).and_then(std::mem::take)
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
