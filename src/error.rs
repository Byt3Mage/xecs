use std::{any::TypeId, fmt::Debug};
use thiserror::Error;

use crate::id::{Entity, Id};

#[derive(Error, Debug)]
pub enum EcsError {
    #[error("{0}")]
    InvalidId(#[from] InvalidEntity),
    #[error("{0}")]
    InvalidPair(#[from] InvalidPair),
    #[error("Component {0} has no associated data (it's a tag)")]
    IsTag(Entity),
    #[error("Component {0} has associated data, can't be used as a tag")]
    IsNotTag(Entity),
    #[error("{0}")]
    MissingComponent(#[from] MissingComponent),
    #[error("Entity {0} is not registered as a component")]
    IdNotComponent(Entity),
    #[error("Type {0} is not registered for this world, must register before use")]
    MissingType(#[from] MissingType),
    #[error("User error: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub type EcsResult<T> = Result<T, EcsError>;

/// Error returned if accessing an [IdRecord](crate::id::id_index::IdRecord) fails
#[derive(Error, Debug)]
#[error("Entity {0} is not alive")]
pub struct InvalidEntity(pub Entity);

#[derive(Error, Debug)]
pub enum InvalidPair {
    #[error("Pair relationship {0} is not valid")]
    Relationship(Entity),
    #[error("Pair target {0} is not valid")]
    Target(Entity),
}

#[derive(Error, Debug)]
#[error("Id {0} is does not have component {1}")]
pub struct MissingComponent(pub Entity, pub Entity);

/// Error returned when a component type is not registered with the world.
#[derive(Error, Debug, Clone, Copy)]
pub struct MissingType {
    /// Human-readable type name for error messages
    pub name: fn() -> &'static str,
    /// Unique type identifier for handling (e.g auto-registration)
    pub type_id: TypeId,
}

impl MissingType {
    #[inline]
    pub const fn new<T: 'static>() -> Self {
        Self {
            name: std::any::type_name::<T>,
            type_id: TypeId::of::<T>(),
        }
    }

    /// Create from an existing `TypeId`. `name` is set to <unknown>
    pub const fn from_id(type_id: TypeId) -> Self {
        Self {
            name: || "<unknown>",
            type_id,
        }
    }
}

impl std::fmt::Display for MissingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "type '{}' is not registered", (self.name)())
    }
}

#[derive(Error, Debug)]
pub enum GetError {
    #[error("{0}")]
    InvalidId(#[from] InvalidEntity),
    #[error("Id: {0} is not a component")]
    IdNotComponent(Entity),
    #[error("Id does not have component {0}")]
    MissingComponent(Entity),
    #[error("Type {0} is not registered for this world, must register before use")]
    MissingType(#[from] MissingType),
}
