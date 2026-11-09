use thiserror::Error;

use crate::{
    ComponentId, IdsExhausted,
    id::{Id, allocator::NotAlive},
};

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    NotAlive(#[from] NotAlive),

    #[error(transparent)]
    IdsExhausted(#[from] IdsExhausted),

    #[error(transparent)]
    MissingComponent(#[from] MissingComponent),

    #[error("{0} does not have relation {1}")]
    MissingRelation(Id, ComponentId),
}

pub type EcsResult<T> = Result<T, Error>;

#[derive(Error, Debug)]
#[error("{id} does not have component {component}")]
pub struct MissingComponent {
    pub id: Id,
    pub component: ComponentId,
}
