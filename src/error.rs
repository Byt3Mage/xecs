use thiserror::Error;

use crate::{
    ComponentId, IdsExhausted, RelationId,
    id::{Id, allocator::NotAlive},
};

#[derive(Error, Debug, Clone, Copy)]
pub enum Error {
    #[error(transparent)]
    NotAlive(#[from] NotAlive),

    #[error(transparent)]
    IdsExhausted(#[from] IdsExhausted),

    #[error(transparent)]
    NoComponent(#[from] NoComponent),

    #[error(transparent)]
    NoRelation(#[from] NoRelation),
}

pub type EcsResult<T> = Result<T, Error>;

#[derive(Error, Debug, Clone, Copy)]
#[error("{id} does not have component {comp}")]
pub struct NoComponent {
    pub id: Id,
    pub comp: ComponentId,
}

#[derive(Error, Debug, Clone, Copy)]
#[error("{src} does not have relation {rel} with {tgt}")]
pub struct NoRelation {
    pub src: Id,
    pub tgt: Id,
    pub rel: RelationId,
}
