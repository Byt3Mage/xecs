use crate::{component::id::ComponentId, query::logical::ScopeId, relation::RelationId};

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

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
    #[error("scope {scope}, access {index} ({name}): &mut on a Read access")]
    WriteOnRead {
        scope: ScopeId,
        index: usize,
        name: &'static str,
    },

    #[error("scope {scope}, access {index} ({name}): &T on a Write access")]
    ReadOnWrite {
        scope: ScopeId,
        index: usize,
        name: &'static str,
    },

    #[error("scope {scope}, access {index} ({name}): query declares optional access")]
    RequiredOnOptional {
        scope: ScopeId,
        index: usize,
        name: &'static str,
    },

    #[error("scope {scope}, access {index} ({name}): query declares required access")]
    OptionalOnRequired {
        scope: ScopeId,
        index: usize,
        name: &'static str,
    },

    #[error("scope {scope}, access {index}: column type is {received}, query declares {expected}")]
    TypeMismatch {
        scope: ScopeId,
        index: usize,
        received: &'static str,
        expected: &'static str,
    },

    #[error("scope {scope}: claims {received} columns, query declares {expected}")]
    ColumnArity {
        scope: ScopeId,
        received: usize,
        expected: usize,
    },

    #[error("scope {scope}: claims {received} joins, query declares {expected}")]
    JoinArity {
        scope: ScopeId,
        received: usize,
        expected: usize,
    },

    #[error("Join<{index}> out of range: query has {count} joins")]
    JoinIndex { index: usize, count: usize },

    #[error("Join<{index}> is not a join from scope {scope} (its from-scope is {actual})")]
    JoinFrom {
        index: usize,
        scope: ScopeId,
        actual: ScopeId,
    },
}
