mod bundle;
mod component;
mod data_structures;
mod ecs;
mod error;
mod id;
mod macros;
mod memory;
mod query;
mod relation;
mod table;
mod table_storage;
mod type_meta;
mod utils;

#[doc(hidden)]
pub use linkme as __linkme;

pub use bundle::{Bundle, Field};
pub use component::{
    Component, ComponentId, ComponentInfo, ComponentKey, STATIC_COMPONENTS, signature::Signature, static_components,
};
pub use ecs::{Ecs, EcsGetError};
pub use error::{EcsResult, Error, NoComponent, NoRelation};
pub use id::{
    Id,
    allocator::{IdsExhausted, NotAlive},
};
pub use query::{
    EcsMismatch, Query, QueryGetError,
    access::{AccessError, AccessSet},
    data::{QueryData, ReadOnlyData, SliceData},
    filter::{QueryFilter, With, Without},
};
pub use relation::{RelateError, Relation, RelationId, RelationKey, STATIC_RELATIONS, EdgeFrom};
pub use type_meta::{HasMeta, TypeMeta};
pub use utils::KeySlot;
