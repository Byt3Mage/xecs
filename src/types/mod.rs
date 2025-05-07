pub(crate) mod type_info;

use crate::{entity::Entity, utils::NoOpHash};
use std::{any::TypeId, collections::HashMap, ops::Deref, rc::Rc};

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

/// Sorted list of ids in an [Arcehetype](crate::storage::table::table)
#[derive(Hash, PartialEq, Eq)]
pub struct IdList(Rc<[Entity]>);

impl Clone for IdList {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl From<Vec<Entity>> for IdList {
    fn from(value: Vec<Entity>) -> Self {
        Self(value.into())
    }
}

impl Deref for IdList {
    type Target = [Entity];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IdList {
    #[inline]
    pub fn ids(&self) -> &[Entity] {
        &self.0
    }

    #[inline]
    pub fn id_count(&self) -> usize {
        self.0.len()
    }

    /// Creates a new sorted type from [Type] and new id.
    ///
    /// Returns [None] if the source type already contains id.
    pub fn try_extend(&self, with: Entity) -> Option<Self> {
        /// Find location where to insert id into type
        fn find_type_insert(ids: &[Entity], to_add: Entity) -> Option<usize> {
            for (i, &id) in ids.iter().enumerate() {
                if id == to_add {
                    return None;
                }
                if id > to_add {
                    return Some(i);
                }
            }

            Some(ids.len())
        }

        let at = find_type_insert(self, with)?;
        let src_array = self.ids();
        let mut dst_array = Vec::with_capacity(src_array.len() + 1);

        dst_array.extend_from_slice(&src_array[..at]);
        dst_array.push(with);
        dst_array.extend_from_slice(&src_array[at..]);

        Some(dst_array.into())
    }
}
