use sparse_storage::{ComponentSparseSet, TagSparseSet};
use std::{alloc::Layout, collections::HashMap, ptr::NonNull, rc::Rc};
use table_index::TableId;

use crate::{
    component::ComponentLocation,
    entity::Entity,
    pointer::{Ptr, PtrMut},
    type_info::TypeInfo,
};

pub mod sparse_set;
pub mod sparse_storage;
pub mod table;
pub mod table_data;
pub mod table_index;

pub enum StorageType {
    Tables,
    Sparse,
}

pub enum Storage {
    SparseTag(TagSparseSet),
    SparseData(ComponentSparseSet),
    Tables(HashMap<TableId, ComponentLocation>),
}
