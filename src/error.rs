use crate::{
    ComponentId,
    component::registry::Unregistered,
    id::{Id, allocator::NotAlive},
};

#[derive(thiserror::Error, Debug)]
pub enum EcsError {
    #[error(transparent)]
    NotAlive(#[from] NotAlive),
    #[error(transparent)]
    UnregisteredStatic(#[from] Unregistered),
    #[error("{id} does not have {component}")]
    MissingComponent { id: Id, component: ComponentId },
}

pub type EcsResult<T> = Result<T, EcsError>;
