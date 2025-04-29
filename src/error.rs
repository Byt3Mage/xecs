use crate::entity::Entity;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug)]
pub enum EcsError {
    EntityIndex(EntityIndexError),
    MissingComponent(MissingComponent),
    UnregisteredType(UnregisteredType),
    UnregisteredComponent(UnregisteredComponent),
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub type EcsResult<T> = Result<T, EcsError>;

/// Error returned if accessing an entity [EntityRecord](crate::entity_index::EntityRecord) fails
#[derive(Error, Debug)]
pub enum EntityIndexError {
    /// [Entity] does not exist, was never created.
    #[error("Entity {0} does not exist")]
    NonExistent(Entity),

    /// [Entity] was created, but is now dead.
    #[error("Entity {0} is dead")]
    NotAlive(Entity),

    /// [Entity] doesn't exist or exists but is not alive.
    #[error("Entity {0} is not valid")]
    NotValid(Entity),
}

impl From<EntityIndexError> for EcsError {
    fn from(err: EntityIndexError) -> Self {
        EcsError::EntityIndex(err)
    }
}

#[derive(Error, Debug)]
#[error("Component {0} is missing from entity {1}")]
pub struct MissingComponent(pub Entity, pub Entity);

impl From<MissingComponent> for EcsError {
    fn from(err: MissingComponent) -> Self {
        EcsError::MissingComponent(err)
    }
}

#[derive(Error, Debug)]
#[error("Type {0} is not registered for this world, must register before use")]
pub struct UnregisteredType(pub &'static str);

impl From<UnregisteredType> for EcsError {
    fn from(err: UnregisteredType) -> Self {
        EcsError::UnregisteredType(err)
    }
}

#[derive(Error, Debug)]
#[error("Entity {0} is not registered as a component")]
pub struct UnregisteredComponent(pub Entity);

impl From<UnregisteredComponent> for EcsError {
    fn from(err: UnregisteredComponent) -> Self {
        EcsError::UnregisteredComponent(err)
    }
}
