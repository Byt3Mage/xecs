use crate::id::Id;
use std::fmt::{Debug, Display};
use thiserror::Error;

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
    UnregisteredType(#[from] UnregisteredTypeErr),
    #[error("Entity {0} is not registered as a component")]
    IdNotComponent(Id),
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
pub struct MissingComponent(pub Id, pub Id);

#[derive(Error, Debug)]
pub struct UnregisteredTypeErr(fn() -> &'static str);

impl Display for UnregisteredTypeErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Type: {} is not registered", (self.0)())
    }
}

#[derive(Error, Debug)]
pub enum GetError {
    #[error("{0}")]
    InvalidId(#[from] InvalidId),
    #[error("Id: {0} is not a component")]
    IdNotComponent(Id),
    #[error("Id does not have component {0}")]
    MissingComponent(Id),
    #[error("Type {0} is not registered for this world, must register before use")]
    UnregisteredType(#[from] UnregisteredTypeErr),
}

pub type GetResult<T> = Result<T, GetError>;

/// Unregistered type error.
pub(crate) const fn unreg_type_err<T>() -> UnregisteredTypeErr {
    UnregisteredTypeErr(std::any::type_name::<T>)
}
