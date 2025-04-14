use std::{fmt::Debug, rc::Rc};

use crate::{component::ComponentView, entity::Entity, id::Id};

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
    Component(&'static str),
    TypeMismatch,
}

impl From<EntityIndexError> for EcsError {
    fn from(value: EntityIndexError) -> Self {
        Self::EntityIndex(value)
    }
}

pub type EcsResult<T> = Result<T, EcsError>;

#[inline]
pub(crate) const fn component_not_registered_err<T>() -> EcsResult<T> {
    Err(EcsError::Component("component not registered."))
}

#[inline]
pub(crate) const fn type_mismatch_err<T>() -> EcsResult<T> {
    Err(EcsError::TypeMismatch)
}