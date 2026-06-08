use std::fmt::Debug;

use crate::{
    id::Id,
    query::QueryBuildError,
    storage::{BorrowMutError, BorrowRefError},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    IdNotComponent(#[from] IdNotComponent),
    #[error("Id {id} does not have component {comp}")]
    MissingComponent { id: Id, comp: Id },
    #[error("Component {id} does not have singleton set")]
    MissingSingleton { id: Id },
    #[error("component `{0}` is not registered with this ecs")]
    UnregisteredComponent(&'static str),
    #[error(transparent)]
    QueryBuild(#[from] QueryBuildError),
    #[error(transparent)]
    BorrowRef(#[from] BorrowRefError),
    #[error(transparent)]
    BorrowMut(#[from] BorrowMutError),
}

pub type EcsResult<T> = Result<T, Error>;

/// Error returned if accessing an [IdRecord](crate::id::manager::IdRecord) fails
#[derive(thiserror::Error, Debug)]
#[error("Id {0} is not alive")]
pub struct InvalidId(pub Id);

/// Error returned if accessing a [ComponentInfo](crate::component::ComponentInfo) fails
#[derive(thiserror::Error, Debug)]
#[error("Id {0} is not registered as a component")]
pub struct IdNotComponent(pub Id);

#[derive(thiserror::Error, Debug)]
#[error("Id {0} is does not have component {1}")]
pub struct MissingComponent(pub Id, pub Id);
