use std::{
    ops::RangeBounds,
    vec::{Drain, Splice},
};

use crate::Id;

pub const fn ix(id: Id) -> usize {
    id.index() as usize
}

pub struct Sparse<T> {
    values: Vec<Option<T>>,
}

impl<T> Sparse<T> {
    pub fn new() -> Self {
        Self { values: vec![] }
    }

    pub fn get(&self, id: Id) -> Option<&T> {
        self.values.get(ix(id)).and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut T> {
        self.values.get_mut(ix(id)).and_then(Option::as_mut)
    }

    pub fn contains(&self, id: Id) -> bool {
        self.values.get(ix(id)).is_some_and(Option::is_some)
    }

    pub fn set(&mut self, id: Id, value: T) {
        let idx = ix(id);
        if idx >= self.values.len() {
            self.values.resize_with(idx + 1, || None);
        }
        self.values[idx] = Some(value);
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

/// A `Vec` indexed by `IdxU32`.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub(crate) struct VecIdxU32<T>(Vec<T>);

impl<T> VecIdxU32<T> {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn len(&self) -> u32 {
        self.0.len() as u32
    }

    pub fn push(&mut self, value: T) {
        debug_assert!(self.len() < u32::MAX, "too many elements");
        self.0.push(value);
    }

    pub fn swap_remove(&mut self, i: u32) -> T {
        self.0.swap_remove(i as usize)
    }

    #[inline]
    pub fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<'_, I::IntoIter>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = T>,
    {
        self.0.splice(range, replace_with)
    }

    pub fn drain<R: RangeBounds<usize>>(&mut self, range: R) -> Drain<'_, T> {
        self.0.drain(range)
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn extend_from_slice(&mut self, other: &[T])
    where
        T: Clone,
    {
        self.0.extend_from_slice(other);
    }
}

impl<T> Default for VecIdxU32<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> std::ops::Deref for VecIdxU32<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for VecIdxU32<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> std::ops::Index<u32> for VecIdxU32<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, i: u32) -> &Self::Output {
        &self.0[i as usize]
    }
}

impl<T> std::ops::IndexMut<u32> for VecIdxU32<T> {
    #[inline(always)]
    fn index_mut(&mut self, i: u32) -> &mut Self::Output {
        &mut self.0[i as usize]
    }
}

impl<T> std::ops::Index<std::ops::Range<u32>> for VecIdxU32<T> {
    type Output = [T];
    #[inline(always)]
    fn index(&self, r: std::ops::Range<u32>) -> &Self::Output {
        &self.0[(r.start as usize)..(r.end as usize)]
    }
}

impl<T> std::ops::IndexMut<std::ops::Range<u32>> for VecIdxU32<T> {
    #[inline(always)]
    fn index_mut(&mut self, r: std::ops::Range<u32>) -> &mut Self::Output {
        &mut self.0[(r.start as usize)..(r.end as usize)]
    }
}
