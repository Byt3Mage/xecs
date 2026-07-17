use crate::{
    ComponentId,
    id::{Id, allocator::NotAlive},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    NotAlive(#[from] NotAlive),
    #[error("{id} does not have {component}")]
    MissingComponent { id: Id, component: ComponentId },
    #[error("static id `{0}` is not registered with this ecs")]
    UnregisteredStatic(u32),
}

pub type EcsResult<T> = Result<T, Error>;
