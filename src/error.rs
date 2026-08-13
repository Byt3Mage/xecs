use crate::{
    ComponentId,
    id::{Id, allocator::NotAlive},
    key::UnregisteredKey,
    relation::storage::RelateError,
};

#[derive(thiserror::Error, Debug)]
pub enum EcsError {
    #[error(transparent)]
    NotAlive(#[from] NotAlive),

    #[error(transparent)]
    UnregisteredKey(#[from] UnregisteredKey),

    #[error("{0} does not have component {1}")]
    MissingComponent(Id, ComponentId),

    #[error(transparent)]
    Relate(#[from] RelateError),
}

pub type EcsResult<T> = Result<T, EcsError>;
