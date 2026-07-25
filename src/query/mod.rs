use std::marker::PhantomData;

use crate::{
    Ecs, Id, ValidateError,
    query::{
        access::{Follows, Select},
        context::{Binds, QueryCtx},
        logical::{LogicalPlan, ScopeId, validate_scope},
        physical::{Fan, MatchedTable, PhysicalPlan, PhysicalScope},
    },
};

pub(crate) mod access;
pub(crate) mod context;
pub(crate) mod error;
pub(crate) mod logical;
pub(crate) mod physical;

#[derive(Debug, Clone)]
pub struct Query {
    physical: PhysicalPlan,
    logical: LogicalPlan,
}

impl Query {
    pub fn new() -> Self {
        todo!()
    }

    /// Re-evaluate the physical plan if out of sync with current ecs state.
    /// `force` triggers re-evaluation regardless of sync status.
    pub fn rematch(&mut self, ecs: &Ecs, force: bool) {
        if force || self.physical.resolved_at != ecs.generation() {
            self.physical = self.logical.lower(ecs);
        }
    }

    #[inline]
    pub fn make_ctx<'w>(&'w self, ecs: &'w Ecs, params: &'w [Id]) -> QueryCtx<'w> {
        QueryCtx {
            ecs,
            plan: &self.physical,
            params,
            binds: Binds::new(self.physical.scopes.len()),
        }
    }

    /// Converts [Query] to a typed [TQuery] with access validation performed.
    #[inline]
    pub fn typed<C: Select, F: Follows>(self, ecs: &Ecs) -> Result<TQuery<C, F>, ValidateError> {
        validate_scope::<C, F>(ecs, &self.logical, 0)?;
        Ok(TQuery { query: self, marker: PhantomData })
    }

    /// Clones [Query] to a typed [TQuery] with access validation performed.
    #[inline(always)]
    pub fn clone_typed<C: Select, F: Follows>(self, ecs: &Ecs) -> Result<TQuery<C, F>, ValidateError> {
        self.clone().typed(ecs)
    }
}

pub struct TQuery<S: Select, F: Follows> {
    query: Query,
    marker: PhantomData<(S, F)>,
}

impl<S: Select, F: Follows> TQuery<S, F> {
    pub fn each(&mut self, ecs: &mut Ecs, params: &[Id], mut f: impl FnMut(Id, S::Row<'_>, F::Get<'_>)) {
        let driver = &self.query.physical.scopes[0];
        let ctx = self.query.make_ctx(ecs, params);

        for mt in &driver.matched_tables {
            let table = &ctx.ecs.tables[mt.id];
            let num_rows = table.num_rows();

            if num_rows == 0 {
                continue;
            }

            let ids = table.ids();
            let columns = S::columns(table, &mt.columns);

            for row in 0..num_rows {
                // SAFETY: `row` is in bounds for this table
                unsafe {
                    let id = ids.add(row as usize).read();
                    if driver.passes_relation_checks(id, &ctx) {
                        f(id, S::row(columns, row), F::get(&ctx, 0, id));
                    }
                }
            }
        }
    }
}

pub struct Follow<'w, S: Select = (), F: Follows = ()> {
    ctx: &'w QueryCtx<'w>,
    index: usize,
    from: Id,
    marker: PhantomData<(S, F)>,
}

impl<'w, S: Select, F: Follows> Follow<'w, S, F> {
    #[inline(always)]
    pub(crate) fn new(ctx: &'w QueryCtx<'w>, index: usize, from: Id) -> Self {
        Self { ctx, index, from, marker: PhantomData }
    }

    #[inline(always)]
    pub fn iter(&mut self) -> FollowIter<'w, S, F> {
        let join = &self.ctx.plan.joins[self.index];
        FollowIter {
            ctx: self.ctx,
            fan: join.fan(self.from, self.ctx),
            scope: &self.ctx.plan.scopes[join.scope],
            scope_id: join.scope,
            marker: PhantomData,
        }
    }

    /// Iterate all follow targets to completion.
    #[inline(always)]
    pub fn each(&mut self, mut func: impl FnMut(Id, S::Row<'w>, F::Get<'w>)) {
        self.iter().for_each(|(i, r, f)| func(i, r, f));
    }

    /// The zero-or-one fast path.
    #[inline(always)]
    pub fn get(&mut self) -> Option<(Id, S::Row<'w>, F::Get<'w>)> {
        self.iter().next()
    }
}

pub struct FollowIter<'w, S: Select, F: Follows> {
    fan: Fan<'w>,
    ctx: &'w QueryCtx<'w>,
    scope: &'w PhysicalScope,
    scope_id: ScopeId,
    marker: PhantomData<(S, F)>,
}

impl<'w, S: Select, F: Follows> Iterator for FollowIter<'w, S, F> {
    type Item = (Id, S::Row<'w>, F::Get<'w>);

    fn next(&mut self) -> Option<Self::Item> {
        self.fan
            .find_map(|id| match_follow(self.ctx, self.scope, id))
            .map(|(id, row, mt)| {
                // SAFETY: row is from a live record; element types were
                // checked against the plan at dispatch; aliasing was proven
                // at lowering or guarded in bind().
                let cols = S::columns(&self.ctx.ecs.tables[mt.id], &mt.columns);
                (id, unsafe { S::row(cols, row) }, F::get(self.ctx, self.scope_id, id))
            })
    }
}

#[inline]
pub fn match_follow<'p>(ctx: &QueryCtx<'_>, scope: &'p PhysicalScope, id: Id) -> Option<(Id, u32, &'p MatchedTable)> {
    let record = ctx.ecs.ids.get(id).expect("edge to dead id: purge broken");
    let matched = scope.resolve(record.table)?;

    // Run relationship and guard checks
    if !scope.passes_relation_checks(id, ctx) || !scope.passes_scope_guards(id, ctx) {
        return None;
    }

    Some((id, record.row, matched))
}
