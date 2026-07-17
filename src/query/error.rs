use crate::{component::ComponentId, query::logical::ScopeId, relation::RelationId};

/// Errors produced while lowering a logical plan against a world.
/// All of these are programming errors in the query or a missing
/// declaration, caught at registration.
#[derive(thiserror::Error, Debug)]
pub enum LowerError {
    /// A reversed Any-check or reversed join on a relationship declared without a reverse index.
    #[error("{0} has no reverse index")]
    NoReverseIndex(RelationId),

    /// A direction marker (`<`) on a symmetric relationship
    #[error("direction marker on symmetric `{0}`")]
    ReversedSymmetric(RelationId),

    /// A write conflict validation cannot guard: e.g. write access to a
    /// component reachable through a multiplying join whose destinations
    /// can collide.
    #[error("conflicting access to `{0}` between scopes {1} and {2}")]
    WriteConflict(ComponentId, ScopeId, ScopeId),
}
