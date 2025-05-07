use crate::entity::Entity;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EcsError {
    #[error("{0}")]
    EntityIndex(EntityIndexError),
    #[error("Component {0} has no associated data (it's a tag)")]
    ComponentHasNoData(Entity),
    #[error("Component {0} has associated data, can't be used as a tag")]
    ComponentHasData(Entity),
    #[error("Entity {0} is missing component {1}")]
    MissingComponent(Entity, Entity),
    #[error("Type {0} is not registered for this world, must register before use")]
    UnregisteredType(&'static str),
    #[error("Entity {0} is not registered as a component")]
    UnregisteredComponent(Entity),
    #[error("TypeMismatch: expected {expected}, got {got}")]
    TypeMismatch {
        expected: &'static str,
        got: &'static str,
    },
    #[error("User error: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub type EcsResult<T> = Result<T, EcsError>;

/// Error returned if accessing an entity [EntityRecord](crate::entity_index::EntityRecord) fails
#[derive(Error, Debug)]
pub enum EntityIndexError {
    /// [Entity] does not exist, was never created.
    #[error("Entity {0} does not exist, was never created")]
    NonExistent(Entity),

    /// [Entity] was created, but is now dead.
    #[error("Entity {0} was created, but is now dead")]
    NotAlive(Entity),

    /// [Entity] doesn't exist or exists but is not alive.
    #[error("Entity {0} doesn't exist or exists but is not alive")]
    NotValid(Entity),
}

impl From<EntityIndexError> for EcsError {
    fn from(err: EntityIndexError) -> Self {
        EcsError::EntityIndex(err)
    }
}

#[inline(always)]
pub fn unregistered_type_err<T, U>() -> EcsResult<U> {
    Err(EcsError::UnregisteredType(std::any::type_name::<T>()))
}
