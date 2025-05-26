// Public modules
pub mod component;
pub mod error;
pub mod flags;
pub mod id;
pub mod macros;
pub mod query;
pub mod storage;
pub mod type_info;
pub mod world;

// Internal modules
mod dynamic_struct;
mod graph;
mod pointer;
mod table_index;
mod utils;
mod world_utils;

// Test modules
#[cfg(test)]
mod tests;
