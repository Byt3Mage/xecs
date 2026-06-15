pub mod access;
pub mod bundle;
pub mod component;
pub mod dynamic_struct;
mod ecs;
pub mod error;
pub mod flags;
mod graph;
pub mod id;
pub mod macros;
pub mod query;
pub mod storage;
mod table_index;
pub mod type_meta;
mod utils;
pub mod validate;

// Re-exports
pub use component::{StaticId, TypedStaticId};
pub use ecs::Ecs;
pub use error::Error;
pub use id::Id;
pub use query::{Query, QueryBuilder};
pub use xecs_macros;
