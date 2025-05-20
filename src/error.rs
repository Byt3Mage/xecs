use crate::id::Id;
use std::fmt::Debug;
use thiserror::Error;

type Msg = &'static str;

#[derive(Error, Debug)]
pub enum EcsError {
    #[error("{0}")]
    EntityIndex(#[from] IndexError),
    #[error("Component {0} has no associated data (it's a tag)")]
    IsTag(Id),
    #[error("Component {0} has associated data, can't be used as a tag")]
    IsNotTag(Id),
    #[error("Entity {entity} is missing component {comp}")]
    MissingComponent { entity: Id, comp: Id },
    #[error("Type {0} is not registered for this world, must register before use")]
    UnregisteredType(Msg),
    #[error("Entity {0} is not registered as a component")]
    IdNotComponent(Id),
    #[error("TypeMismatch: expected {exp}, got {got}")]
    TypeMismatch { exp: Msg, got: Msg },
    #[error("User error: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub type EcsResult<T> = Result<T, EcsError>;

/// Error returned if accessing an entity [EntityRecord](crate::entity_index::EntityRecord) fails
#[derive(Error, Debug)]
pub enum IndexError {
    /// [Entity] does not exist, was never created.
    #[error("Entity {0} does not exist, was never created")]
    NonExistent(Id),

    /// [Entity] was created, but is now dead.
    #[error("Entity {0} was created, but is now dead")]
    NotAlive(Id),

    /// [Entity] doesn't exist or exists but is not alive.
    #[error("Entity {0} doesn't exist or exists but is not alive")]
    NotValid(Id),
}

#[inline(always)]
pub fn unregistered_type<T>() -> EcsError {
    EcsError::UnregisteredType(std::any::type_name::<T>())
}

#[inline(always)]
pub const fn missing_component(entity: Id, comp: Id) -> EcsError {
    EcsError::MissingComponent { entity, comp }
}
