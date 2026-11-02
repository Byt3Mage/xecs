use crate::ComponentId;

/// Errors produced while lowering a logical plan against a world.
#[derive(thiserror::Error, Debug)]
#[error("dupicate access for {component} at positions `{first}` and `{second}`")]
pub struct PlanError {
    pub component: ComponentId,
    pub first: u32,
    pub second: u32,
}

#[derive(thiserror::Error, Debug)]
pub enum AccessError {
    #[error("access[{index}]: &mut on a Read access")]
    WriteOnRead { index: u32 },

    #[error("access[{index}]: &T on a Write access")]
    ReadOnWrite { index: u32 },

    #[error("access[{index}]: &(mut)T on an optional access")]
    RequiredOnOptional { index: u32 },

    #[error("access[{index}]: Option<&(mut)T> on a required access")]
    OptionalOnRequired { index: u32 },

    #[error("access[{index}]: column type mismatch")]
    TypeMismatch { index: u32 },

    #[error("selects {received} columns, query declares {expected}")]
    ArityMismatch { received: u32, expected: u32 },
}
