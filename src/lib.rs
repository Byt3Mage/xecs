mod bundle;
mod component;
mod dynamic_struct;
mod ecs;
mod error;
mod flags;
mod graph;
mod id;
mod macros;
mod query;
mod relation;
mod storage;
mod table_index;
mod type_meta;
mod utils;
mod validate;

// Re-exports
pub use component::{ComponentHooks, ComponentId, StaticId, TypedStaticId};
pub use ecs::Ecs;
pub use error::Error;
pub use id::Id;
pub use type_meta::TypeMeta;
pub use xecs_macros::{Component, components};
