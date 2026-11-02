use std::rc::Rc;

use crate::{
    Ecs, Id,
    component::ComponentId,
    query::{
        access::{Access, AccessMode, Select},
        context::Binds,
        error::{AccessError, PlanError},
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
            scopes: vec![LogicalScope::default()],
            follows: vec![],
            current: 0,
        }
    }
    pub fn select(mut self, id: ComponentId, mode: AccessMode, optional: bool) -> Self {
        self.access.push(Access { id, mode, optional });
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

    pub fn follow<F>(mut self, relation: Relation, f: impl FnOnce(Self) -> Self) -> Self {
        self.scopes[self.current].follows.push(self.follows.len());

        let prev = self.current;

        self.current = self.scopes.len();
        self.follows.push(LogicalFollow { relation, scope: self.current });
        self.scopes.push(LogicalScope::default());
        self = f(self);
        self.current = prev;
        self
    }

    pub fn build(self) -> Result<LogicalPlan, PlanError> {
        check_disjoint(&self.access)?;
        Ok(LogicalPlan {
            access: self.access.into(),
            scopes: self.scopes.into(),
            follows: self.follows.into(),
        })
    }
}

fn check_disjoint(access: &[Access]) -> Result<(), PlanError> {
    for (i, a) in access.iter().enumerate() {
        for (j, b) in access[i + 1..].iter().enumerate() {
            if a.id == b.id {
                return Err(PlanError {
                    component: a.id,
                    first: i as u32,
                    second: (i + 1 + j) as u32,
                });
            }
        }
    }
    Ok(())
}

pub fn validate_access<S: Select>(ecs: &Ecs, plan: &LogicalPlan) -> Result<(), AccessError> {
    let requests = S::ACCESSES;
    let declared = &plan.access;

    let received = requests.len() as u32;
    let expected = declared.len() as u32;

    if received != expected {
        return Err(AccessError::ArityMismatch { received, expected });
    }

    for (index, (r, d)) in requests.iter().zip(declared.iter()).enumerate() {
        let index = index as u32;

        match (r.mode, d.mode) {
            (AccessMode::Read, AccessMode::Write) => return Err(AccessError::ReadOnWrite { index }),
            (AccessMode::Write, AccessMode::Read) => return Err(AccessError::WriteOnRead { index }),
            _ => {}
        }

        match (r.optional, d.optional) {
            (true, false) => return Err(AccessError::OptionalOnRequired { index }),
            (false, true) => return Err(AccessError::RequiredOnOptional { index }),
            _ => {}
        }

        if ecs.components[d.id].meta.type_id != r.type_id {
            return Err(AccessError::TypeMismatch { index });
        }
    }
    Ok(())
}
