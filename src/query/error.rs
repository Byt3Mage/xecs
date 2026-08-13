use crate::relation::id::RelationId;

/// Errors produced while lowering a logical plan against a world.
/// All of these are programming errors in the query or a missing
/// declaration, caught at registration.
#[derive(thiserror::Error, Debug)]
pub enum PlanValidationError {
    /// A reversed Any-check or reversed join on a relationship declared without a reverse index.
    #[error("{0} has no reverse index")]
    NoReverseIndex(RelationId),

    /// A direction marker (`<`) on a symmetric relationship
    #[error("direction marker on symmetric `{0}`")]
    ReversedSymmetric(RelationId),
}

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
    #[error("access {index} ({name}): &mut on a Read access")]
    WriteOnRead { index: usize, name: &'static str },

    #[error("access {index} ({name}): &T on a Write access")]
    ReadOnWrite { index: usize, name: &'static str },

    #[error("access {index} ({name}): query declares optional access")]
    RequiredOnOptional { index: usize, name: &'static str },

    #[error("access {index} ({name}): query declares required access")]
    OptionalOnRequired { index: usize, name: &'static str },

    #[error("access {index}: column type mismatch")]
    TypeMismatch { index: usize },

    #[error("claims {received} columns, query declares {expected}")]
    ColumnArity { received: usize, expected: usize },
}
