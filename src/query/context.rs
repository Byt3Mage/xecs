use std::cell::Cell;

use crate::{
    Ecs, Id,
    query::{logical::ScopeId, physical::PhysicalPlan},
};

pub struct QueryCtx<'w> {
    pub(crate) ecs: &'w Ecs,
    pub(crate) plan: &'w PhysicalPlan,
    pub(crate) params: &'w [Id],
    pub(crate) binds: Binds,
}

pub(crate) struct Binds(Box<[Cell<Id>]>);

impl Binds {
    /// One slot per tracked scope. Initial value is never read: a slot
    /// is only readable via probes/guards on LATER scopes, and lowering
    /// guarantees the tracked scope binds (and writes) first.
    pub(crate) fn new(count: usize) -> Self {
        Self(vec![Cell::new(Id::NULL); count].into())
    }

    #[inline(always)]
    pub(crate) fn set(&self, scope: ScopeId, id: Id) {
        self.0[scope].set(id);
    }

    #[inline(always)]
    pub(crate) fn get(&self, scope: ScopeId) -> Id {
        self.0[scope].get()
    }
}
