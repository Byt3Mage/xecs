mod component;
mod dynamic_struct;
mod ecs;
mod error;
mod graph;
mod id;
mod inline_vec;
mod query;
mod relation;
mod storage;
mod table_index;
mod type_meta;

#[doc(hidden)]
pub use linkme as __linkme;

pub use component::{
    ComponentHooks,
    id::{ComponentId, STATIC_COMPONENTS, StaticId, TypedStaticId, UntypedStaticId, static_id_count},
};
pub use ecs::Ecs;
pub use error::EcsError;
pub use id::Id;
pub use inline_vec::InlineVec;
pub use query::{
    Follow, FollowIter, Query, TQuery,
    access::{Access, AccessMode, Follows, Select},
    error::{LowerError, ValidationError},
    logical::{LogicalPlan, PlanBuilder},
};
pub use type_meta::TypeMeta;
pub use xecs_macros::Component;
