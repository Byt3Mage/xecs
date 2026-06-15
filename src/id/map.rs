use std::{
    array,
    ops::{Index, IndexMut},
};

use ahash::AHashMap;

use crate::id::Id;

pub const HI_COMPONENT_ID: usize = 256;

pub struct IdMap<T> {
    id_index_lo: Box<[Option<T>; HI_COMPONENT_ID]>,
    id_index_hi: AHashMap<Id, T>,
}

impl<T> IdMap<T> {
    pub fn new() -> Self {
        Self {
            id_index_lo: Box::new(array::from_fn(|_| None)),
            id_index_hi: AHashMap::new(),
        }
    }

    pub fn contains(&self, id: Id) -> bool {
        match self.id_index_lo.get(id.raw() as usize) {
            Some(v) => v.is_some(),
            _ => self.id_index_hi.contains_key(&id),
        }
    }

    pub fn get(&self, id: Id) -> Option<&T> {
        match self.id_index_lo.get(id.raw() as usize) {
            Some(v) => v.as_ref(),
            _ => self.id_index_hi.get(&id),
        }
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut T> {
        match self.id_index_lo.get_mut(id.raw() as usize) {
            Some(v) => v.as_mut(),
            _ => self.id_index_hi.get_mut(&id),
        }
    }

    pub fn insert(&mut self, id: Id, val: T) -> Option<T> {
        match self.id_index_lo.get_mut(id.raw() as usize) {
            Some(v) => v.replace(val),
            None => self.id_index_hi.insert(id, val),
        }
    }

    pub fn remove(&mut self, id: Id) -> Option<T> {
        match self.id_index_lo.get_mut(id.raw() as usize) {
            Some(v) => v.take(),
            None => self.id_index_hi.remove(&id),
        }
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.id_index_lo
            .iter_mut()
            .flatten()
            .chain(self.id_index_hi.values_mut())
    }
}

impl<T> Index<Id> for IdMap<T> {
    type Output = T;

    fn index(&self, index: Id) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<Id> for IdMap<T> {
    fn index_mut(&mut self, index: Id) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}
