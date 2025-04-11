use std::rc::Rc;

use crate::{entity::Entity, id::Id};

/// Error returned if accessing an entity [Record](crate::internals::Record) fails
#[derive(Debug)]
pub enum EntityIndexError {
    /// [EntityId] does not exist, was never created.
    NonExistent(Entity),

    /// [EntityId] was created, but is now dead.
    NotAlive(Entity),

    /// [EntityId] exists but is not alive.
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
}

impl From<EntityIndexError> for EcsError {
    fn from(value: EntityIndexError) -> Self {
        Self::EntityIndex(value)
    }
}

pub type EcsResult<T> = Result<T, EcsError>;