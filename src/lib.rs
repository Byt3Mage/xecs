mod bundle;
mod component;
mod data_structures;
mod ecs;
mod error;
mod id;
mod inline_vec;
mod key;
mod macros;
mod memory;
mod query;
mod relation;
mod table;
mod table_storage;
mod type_meta;

#[doc(hidden)]
pub use linkme as __linkme;

pub use bundle::{Bundle, Field};
pub use component::{
    Component, ComponentId, ComponentInfo, ComponentKey, ComponentRegisterError, KeySlot, STATIC_COMPONENTS,
    signature::Signature, static_components,
};
pub use ecs::Ecs;
pub use error::{EcsError, EcsResult};
pub use id::{
    Id,
    allocator::{IdsExhausted, NotAlive},
};
pub use inline_vec::InlineVec;
pub use key::{RelationKey, STATIC_RELATIONS};
pub use query::{
    Follow, FollowIter, Query, TQuery,
    access::{Access, AccessMode, Select},
    error::{AccessError, PlanError},
    logical::{LogicalPlan, PlanBuilder},
};
pub use relation::{
    RelationRegisterError,
    id::{IntoRelationId, RelationId},
    storage::Shape,
};
pub use type_meta::{HasMeta, TypeMeta};
