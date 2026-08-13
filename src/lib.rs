mod component;
mod data_structures;
mod ecs;
mod error;
mod graph;
mod id;
mod inline_vec;
mod key;
mod macros;
mod proto;
mod query;
mod relation;
mod storage;
mod table_index;
mod type_meta;

#[doc(hidden)]
pub use linkme as __linkme;

pub use component::{ComponentRegisterError, id::ComponentId};
pub use ecs::Ecs;
pub use error::EcsError;
pub use id::Id;
pub use inline_vec::InlineVec;
pub use key::{ComponentKey, RelationKey, STATIC_COMPONENTS, STATIC_RELATIONS, UntypedKey};
pub use query::{
    Follow, FollowIter, Query, TQuery,
    access::{Access, AccessMode, Select},
    error::{PlanValidationError, ValidationError},
    logical::{LogicalPlan, PlanBuilder},
};
pub use relation::{RelationRegisterError, id::RelationId, storage::Shape};
pub use type_meta::TypeMeta;
