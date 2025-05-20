pub mod type_info;

use crate::{id::Id, utils::NoOpHash};
use std::{any::TypeId, collections::HashMap, fmt::Display, ops::Deref, rc::Rc};

pub struct TypeMap<V> {
    types: HashMap<TypeId, V, NoOpHash>,
}

impl<V> TypeMap<V> {
    pub fn new() -> Self {
        Self {
            types: HashMap::default(),
        }
    }

    #[inline]
    pub fn get<T: 'static>(&self) -> Option<&V> {
        self.types.get(&TypeId::of::<T>())
    }

    #[inline]
    pub fn insert<T: 'static>(&mut self, val: V) {
        self.types.insert(TypeId::of::<T>(), val);
    }

    pub fn remove<T: 'static>(&mut self) {
        self.types.remove(&TypeId::of::<T>());
    }

    #[inline]
    pub fn contains<T: 'static>(&self) -> bool {
        self.types.contains_key(&TypeId::of::<T>())
    }
}

/// Sorted list of ids in a [Table](crate::storage::table::Table)
#[derive(Hash, PartialEq, Eq)]
pub struct IdList(Rc<[Id]>);

impl Display for IdList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Clone for IdList {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl From<Vec<Id>> for IdList {
    fn from(mut value: Vec<Id>) -> Self {
        Self({
            value.sort();
            value.into()
        })
    }
}

impl<const N: usize> From<[Id; N]> for IdList {
    fn from(mut value: [Id; N]) -> Self {
        Self({
            value.sort();
            value.into()
        })
    }
}

impl Deref for IdList {
    type Target = [Id];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IdList {
    #[inline]
    pub fn ids(&self) -> &[Id] {
        &self.0
    }

    #[inline]
    pub fn id_count(&self) -> usize {
        self.0.len()
    }

    /// Creates a new sorted list from [Self] and new id.
    ///
    /// Returns [None] if the source type already contains id.
    pub fn try_extend(&self, with: Id) -> Option<Self> {
        match self.binary_search(&with) {
            Ok(_) => None,
            Err(pos) => {
                let mut new_list = Vec::with_capacity(pos);
                new_list.extend_from_slice(&self[..pos]);
                new_list.push(with);
                new_list.extend_from_slice(&self[pos..]);
                Some(new_list.into())
            }
        }
    }
}
