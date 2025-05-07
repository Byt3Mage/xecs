// Public modules
pub mod component;
pub mod entity;
pub mod entity_view;
pub mod error;
pub mod flags;
pub mod macros;
pub mod storage;
pub mod types;
pub mod world;

// Internal modules
mod dynamic_struct;
mod entity_index;
mod graph;
mod pointer;
mod relationships;
mod table_index;
mod utils;
mod world_utils;

// Test modules
#[cfg(test)]
mod tests;
