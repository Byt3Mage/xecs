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
mod utils;

pub use component::{ComponentHooks, ComponentId, StaticId, TypedStaticId};
pub use ecs::Ecs;
pub use error::Error;
pub use id::Id;
pub use inline_vec::InlineVec;
pub use query::{
    Follow, FollowIter, Query, TQuery,
    access::Follows,
    error::{LowerError, ValidateError},
    logical::PlanBuilder,
};
pub use type_meta::TypeMeta;
pub use xecs_macros::{Component, components};
