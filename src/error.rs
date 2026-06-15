use std::fmt::Debug;

use crate::{id::Id, validate::WriteConflict};

/// Error returned if accessing an [IdRecord](crate::id::manager::IdRecord) fails
#[derive(thiserror::Error, Debug)]
#[error("Id {0} is not alive")]
pub struct InvalidId(pub Id);

/// Error returned if accessing a [ComponentInfo](crate::component::ComponentInfo) fails
#[derive(thiserror::Error, Debug)]
#[error("Id {0} is not registered as a component")]
pub struct IdNotComponent(pub Id);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    IdNotComponent(#[from] IdNotComponent),
    #[error("Id {id} does not have component {comp}")]
    MissingComponent { id: Id, comp: Id },
    #[error("component {id} does not have resource set")]
    MissingResource { id: Id },
    #[error("component `{0}` is not registered with this ecs")]
    UnregisteredStatic(&'static str),
    #[error(transparent)]
    WriteConflict(#[from] WriteConflict),
}

pub type EcsResult<T> = Result<T, Error>;
