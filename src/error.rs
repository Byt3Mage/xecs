use crate::id::Id;
use std::fmt::Debug;
use thiserror::Error;

type TypeName = &'static str;

#[derive(Error, Debug)]
pub enum EcsError {
    #[error("{0}")]
    InvalidId(#[from] InvalidId),
    #[error("{0}")]
    InvalidPair(#[from] InvalidPair),
    #[error("Component {0} has no associated data (it's a tag)")]
    IsTag(Id),
    #[error("Component {0} has associated data, can't be used as a tag")]
    IsNotTag(Id),
    #[error("{0}")]
    MissingComponent(#[from] MissingComponent),
    #[error("Type {0} is not registered for this world, must register before use")]
    UnregisteredType(TypeName),
    #[error("Entity {0} is not registered as a component")]
    IdNotComponent(Id),
    #[error("TypeMismatch: expected {exp}, got {got}")]
    TypeMismatch { exp: TypeName, got: TypeName },
    #[error("User error: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub type EcsResult<T> = Result<T, EcsError>;

/// Error returned if accessing an [IdRecord](crate::id::id_index::IdRecord) fails
#[derive(Error, Debug)]
#[error("Entity {0} is not alive")]
pub struct InvalidId(pub Id);

#[derive(Error, Debug)]
pub enum InvalidPair {
    #[error("Pair relationship {0} is not valid")]
    Relationship(Id),
    #[error("Pair target {0} is not valid")]
    Target(Id),
}

#[derive(Error, Debug)]
#[error("Id {0} is does not have component {1}")]
pub struct MissingComponent(Id, Id);

#[inline(always)]
pub fn unregistered_type<T>() -> EcsError {
    EcsError::UnregisteredType(std::any::type_name::<T>())
}
