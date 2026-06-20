mod access;
mod bundle;
pub mod component;
mod dynamic_struct;
mod ecs;
mod error;
mod flags;
mod graph;
mod id;
mod macros;
mod query;
mod storage;
mod table_index;
mod type_meta;
mod utils;
mod validate;

// Re-exports
pub use ecs::Ecs;
pub use error::Error;
pub use id::Id;
pub use query::{CombinedQuery, Query, QueryBuilder, combine};
pub use storage::StorageType;
pub use type_meta::TypeMeta;
pub use validate::WriteAccessError;
pub use xecs_macros::{Component, components};
