use std::rc::Rc;

use crate::{entity::Entity, id::Id};

/// Error returned if accessing an entity [EntityRecord](crate::entity_index::EntityRecord) fails
#[derive(Debug)]
pub enum EntityIndexError {
    /// [Entity] does not exist, was never created.
    NonExistent(Entity),

    /// [Entity] was created, but is now dead.
    NotAlive(Entity),

    /// [Entity] doesn't exist or exists but is not alive.
    NotValid(Entity)
}

#[derive(Debug)]
pub enum EntityCreateError {
    NameInUse(Id, Rc<str>),
}

#[derive(Debug)]
pub enum EcsError {
    EntityIndex(EntityIndexError),
    EntityCreate(EntityCreateError),
    ComponentCreate(&'static str),
}

impl From<EntityIndexError> for EcsError {
    fn from(value: EntityIndexError) -> Self {
        Self::EntityIndex(value)
    }
}

pub type EcsResult<T> = Result<T, EcsError>;