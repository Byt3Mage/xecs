use crate::entity::Entity;
use std::fmt::Debug;
use thiserror::Error;

type Msg = &'static str;

#[derive(Error, Debug)]
pub enum EcsError {
    #[error("{0}")]
    EntityIndex(#[from] EntityIndexError),
    #[error("Component {0} has no associated data (it's a tag)")]
    IsTag(Entity),
    #[error("Component {0} has associated data, can't be used as a tag")]
    IsNotTag(Entity),
    #[error("Entity {entity} is missing component {id}")]
    MissingComponent { entity: Entity, id: Entity },
    #[error("Type {0} is not registered for this world, must register before use")]
    UnregisteredType(Msg),
    #[error("Entity {0} is not registered as a component")]
    UnregisteredComponent(Entity),
    #[error("TypeMismatch: expected {exp}, got {got}")]
    TypeMismatch { exp: Msg, got: Msg },
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

#[inline(always)]
pub fn unregistered_type<T>() -> EcsError {
    EcsError::UnregisteredType(std::any::type_name::<T>())
}
