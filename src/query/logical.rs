use std::{collections::HashMap, rc::Rc};

use crate::{
    Ecs, Id,
    component::id::ComponentId,
    query::{
        access::{Access, AccessMode, Select},
        context::Binds,
        error::ValidationError,
    },
    relation::id::RelationId,
};

pub(crate) type ScopeId = usize;
pub(crate) type FollowId = usize;

#[derive(Debug, Clone)]
pub struct LogicalPlan {
    pub(crate) access: Rc<[Access]>,
    pub(crate) scopes: Rc<[LogicalScope]>,
    pub(crate) follows: Rc<[LogicalFollow]>,
}

#[derive(Debug, Clone, Default)]
pub struct LogicalScope {
    pub filter: Filter,
    pub checks: Vec<LogicalCheck>,
    pub follows: Vec<FollowId>,
}

#[derive(Debug, Clone, Copy)]
pub struct LogicalFollow {
    pub relation: Relation,
    pub scope: ScopeId,
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub with: Vec<ComponentId>,
    pub without: Vec<ComponentId>,
}

#[derive(Copy, Clone, Debug)]
pub struct Relation {
    pub id: RelationId,
    pub target: Option<IdSource>,
    pub direction: Direction,
}

/// How this follow walks the index. Fixed at lowering: `Symmetric` is
/// selected from the relation's declared topology, and `Reverse` has
/// already been proven to have a secondary index.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
}

/// Where a comparison entity comes from at row time. Shared by probes
/// and joins.
#[derive(Debug, Copy, Clone)]
pub enum IdSource {
    /// #id: fixed at build.
    Fixed(Id),
    /// $n: supplied at dispatch, resolved once per execution.
    Param(u8),
    /// label: read from bindings when the probe runs.
    Scope(u8),
}

impl IdSource {
    #[inline(always)]
    pub(crate) fn resolve(&self, binds: &Binds, params: &[Id]) -> Id {
        match *self {
            IdSource::Fixed(i) => i,
            IdSource::Param(n) => params[n as usize],
            IdSource::Scope(d) => binds.get(d),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct LogicalCheck {
    pub relation: Relation,
    pub negated: bool,
}

pub struct PlanBuilder {
    access: Vec<Access>,
    scopes: Vec<LogicalScope>,
    follows: Vec<LogicalFollow>,
    labels: HashMap<Rc<str>, ScopeId>,
    current: ScopeId,
}

impl Default for PlanBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanBuilder {
    pub fn new() -> Self {
        Self {
            access: vec![],
            scopes: vec![],
            follows: vec![],
            labels: HashMap::new(),
            current: 0,
        }
    }
    pub fn access(mut self, access: Access) -> Self {
        self.access.push(access);
        self
    }

    pub fn with(mut self, c: ComponentId) -> Self {
        self.scopes[self.current].filter.with.push(c);
        self
    }

    pub fn without(mut self, c: ComponentId) -> Self {
        self.scopes[self.current].filter.without.push(c);
        self
    }

    pub fn check(mut self, check: LogicalCheck) -> Self {
        self.scopes[self.current].checks.push(check);
        self
    }

    pub fn follow<F>(mut self, relation: Relation, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        self.scopes[self.current].follows.push(self.follows.len());

        let prev = self.current;

        self.current = self.scopes.len();
        self.follows.push(LogicalFollow { relation, scope: self.current });
        self.scopes.push(LogicalScope::default());
        self = f(self);
        self.current = prev;
        self
    }

    pub fn build(self) -> LogicalPlan {
        LogicalPlan {
            access: self.access.into(),
            scopes: self.scopes.into(),
            follows: self.follows.into(),
        }
    }
}

pub fn validate_access<S>(ecs: &Ecs, plan: &LogicalPlan) -> Result<(), ValidationError>
where
    S: Select,
{
    let requests = S::describe();
    let declared = &plan.access;

    if requests.len() != declared.len() {
        return Err(ValidationError::ColumnArity { received: requests.len(), expected: declared.len() });
    }

    for (index, (req, dec)) in requests.iter().zip(declared.iter()).enumerate() {
        let name = req.type_name;

        use AccessMode::*;
        match (req.mode, dec.mode) {
            (Read, Read) | (Write, Write) => {}
            (Read, Write) => return Err(ValidationError::ReadOnWrite { index, name }),
            (Write, Read) => return Err(ValidationError::WriteOnRead { index, name }),
        }

        match (req.optional, dec.optional) {
            (true, true) | (false, false) => {}
            (true, false) => return Err(ValidationError::OptionalOnRequired { index, name }),
            (false, true) => return Err(ValidationError::RequiredOnOptional { index, name }),
        }

        let meta = &ecs.components.get(dec.id).meta;
        if meta.type_id().is_none_or(|id| id == req.type_id) {
            return Err(ValidationError::TypeMismatch { index });
        }
    }
    Ok(())
}
