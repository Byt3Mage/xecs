use std::marker::PhantomData;

use crate::{
    Ecs, Id, ValidationError,
    query::{
        access::{Follows, Select},
        context::{Binds, QueryCtx},
        logical::{FollowId, LogicalPlan, ScopeId, validate_access},
        lowering::lower_plan,
        physical::{Fan, PhysicalPlan, PhysicalScope},
    },
};

pub(crate) mod access;
pub(crate) mod context;
pub(crate) mod error;
pub(crate) mod logical;
pub(crate) mod lowering;
pub(crate) mod physical;

#[derive(Debug, Clone)]
pub struct Query {
    physical: PhysicalPlan,
    logical: LogicalPlan,
}

impl Query {
    pub fn new(ecs: &Ecs, logical: LogicalPlan) -> Self {
        Self { physical: lower_plan(ecs, &logical), logical }
    }

    /// Re-evaluate the physical plan if out of sync with current ecs state.
    /// `force` triggers re-evaluation regardless of sync status.
    pub fn rematch(&mut self, ecs: &Ecs, force: bool) {
        if force || self.physical.resolved_at != ecs.generation() {
            self.physical = lower_plan(ecs, &self.logical);
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
    pub fn typed<S: Select, F: Follows>(self, ecs: &Ecs) -> Result<TQuery<S, F>, ValidationError> {
        validate_access::<S>(ecs, &self.logical)?;
        Ok(TQuery { query: self, marker: PhantomData })
    }
}

pub struct TQuery<S: Select, F: Follows> {
    query: Query,
    marker: PhantomData<(S, F)>,
}

impl<S: Select, F: Follows> TQuery<S, F> {
    pub fn each(&mut self, ecs: &mut Ecs, params: &[Id], mut f: impl FnMut(Id, S::Row<'_>, F::Get<'_>)) {
        let ctx = self.query.make_ctx(ecs, params);
        let plan = &self.query.physical;
        let scope = &self.query.physical.scopes[0];

        for mt in plan.tables.iter() {
            let table = &ecs.tables[mt.id];
            let rows = table.num_rows();

            if rows == 0 {
                continue;
            }

            let ids = table.ids();
            let columns = S::columns(table, &mt.columns);

            'table: for row in 0..rows {
                // SAFETY: `row` is in bounds for this table
                unsafe {
                    let id = ids.add(row as usize).read();

                    if !scope.check_relations(id, &ctx) {
                        continue 'table;
                    }

                    f(id, S::row(columns, row), F::get(&ctx, 0, id, &scope.follows));
                }
            }
        }
    }
}

pub struct Follow<'w, F: Follows> {
    ctx: &'w QueryCtx<'w>,
    from: Id,
    index: FollowId,
    marker: PhantomData<F>,
}

impl<'w, F: Follows> Follow<'w, F> {
    #[inline(always)]
    pub(crate) fn new(ctx: &'w QueryCtx<'w>, from: Id, index: usize) -> Self {
        Self { ctx, from, index, marker: PhantomData }
    }

    #[inline(always)]
    pub fn iter(&mut self) -> FollowIter<'w, F> {
        let follow = &self.ctx.plan.follows[self.index];
        FollowIter {
            ctx: self.ctx,
            fan: follow.fan(self.from, self.ctx),
            scope: &self.ctx.plan.scopes[follow.scope],
            scope_id: follow.scope,
            marker: PhantomData,
        }
    }

    /// Iterate all follow targets to completion.
    #[inline(always)]
    pub fn each(&mut self, func: impl FnMut((Id, F::Get<'_>))) {
        self.iter().for_each(func);
    }

    /// The zero-or-one fast path.
    #[inline(always)]
    pub fn get(&mut self) -> Option<(Id, F::Get<'_>)> {
        self.iter().next()
    }
}

pub struct FollowIter<'w, F: Follows> {
    fan: Fan<'w>,
    ctx: &'w QueryCtx<'w>,
    scope: &'w PhysicalScope,
    scope_id: ScopeId,
    marker: PhantomData<F>,
}

impl<'w, F: Follows> Iterator for FollowIter<'w, F> {
    type Item = (Id, F::Get<'w>);

    fn next(&mut self) -> Option<Self::Item> {
        let ctx = self.ctx;
        let scope = self.scope;
        let scope_id = self.scope_id;
        self.fan.find_map(
            |id| match scope.check_relations(id, ctx) && scope.passes_filter(ctx, id) {
                true => Some((id, F::get(ctx, scope_id, id, &scope.follows))),
                false => None,
            },
        )
    }
}
