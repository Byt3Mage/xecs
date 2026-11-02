use thiserror::Error;

use crate::{
    ComponentId, IdsExhausted,
    id::{Id, allocator::NotAlive},
    relation::storage::RelateError,
};

#[derive(Error, Debug)]
pub enum EcsError {
    #[error(transparent)]
    NotAlive(#[from] NotAlive),

    #[error(transparent)]
    IdsExhausted(#[from] IdsExhausted),

    #[error("{0} does not have component {1}")]
    MissingComponent(Id, ComponentId),

    #[error(transparent)]
    Relate(#[from] RelateError),
}

pub type EcsResult<T> = Result<T, EcsError>;
