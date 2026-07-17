use crate::{Id, component::ComponentId, relation::RelationId};

pub type ScopeId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Read,
    Write,
}

/// One SELECT term: data access with a mode
#[derive(Debug, Clone, Copy)]
pub struct Access {
    pub id: ComponentId,
    pub mode: AccessMode,
    pub optional: bool,
}

#[derive(Copy, Clone, Debug)]
pub struct Relation {
    pub id: RelationId,
    pub target: RelTarget,
    pub reversed: bool,
}

#[derive(Copy, Clone, Debug)]
pub struct RelCheck {
    pub relation: Relation,
    pub negated: bool,
}

#[derive(Copy, Clone, Debug)]
pub enum RelTarget {
    /// (Rel, _): any target at all.
    Any,
    /// (Rel, #001): a specific id, fixed at build time.
    Id(Id),
    /// (Rel, $n): a specific id, supplied at execution.
    Param(u16),
    /// (Rel, label): unification with an already-bound scope.
    Label(ScopeId),
}

#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub access: Vec<Access>,
    pub with: Vec<ComponentId>,
    pub without: Vec<ComponentId>,
    pub rel_check: Vec<RelCheck>,
}

#[derive(Copy, Clone, Debug)]
pub struct Join {
    pub relation: Relation,
    pub optional: bool,
    pub from: ScopeId,
    pub dest: ScopeId,
}

#[derive(Debug)]
pub struct LogicalPlan {
    pub scopes: Vec<Scope>,
    pub joins: Vec<Join>,
    pub labels: Vec<(Box<str>, ScopeId)>,
}

impl Access {
    #[inline]
    pub fn writes(&self) -> bool {
        matches!(self.mode, AccessMode::Write)
    }

    #[inline]
    pub fn reads(&self) -> bool {
        matches!(self.mode, AccessMode::Read)
    }
}

impl Relation {
    #[inline(always)]
    pub fn as_reversed(&self) -> Option<Self> {
        self.reversed.then(|| *self)
    }
}

impl RelTarget {
    #[inline]
    pub fn is_any(&self) -> bool {
        matches!(self, Self::Any)
    }
}
