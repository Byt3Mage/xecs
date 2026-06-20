use std::fmt::Debug;

use crate::{
    component::NotComponent,
    id::{Id, allocator::NotAlive},
    validate::WriteAccessError,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    NotAlive(#[from] NotAlive),
    #[error(transparent)]
    NotComponent(#[from] NotComponent),
    #[error("Id {id} does not have component {comp}")]
    MissingComponent { id: Id, comp: Id },
    #[error("resource value is not set for component {id}")]
    MissingResource { id: Id },
    #[error("component `{0}` is not registered with this ecs")]
    UnregisteredComponent(&'static str),
    #[error(transparent)]
    WriteConflict(#[from] WriteAccessError),
}

pub type EcsResult<T> = Result<T, Error>;
