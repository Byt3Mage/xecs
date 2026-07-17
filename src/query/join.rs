use std::marker::PhantomData;

use crate::{
    Ecs, Id,
    query::{
        QueryCtx,
        iter::{Columns, Joins},
        physical::MatchedTable,
    },
    table_index::TableId,
};

pub struct Join<C: Columns, J: Joins> {
    index: usize,
    from: Id,
    marker: PhantomData<(C, J)>,
}

impl<C: Columns, J: Joins> Join<C, J> {
    #[inline(always)]
    pub(crate) fn new(index: usize, from: Id) -> Self {
        Self { index, from, marker: PhantomData }
    }

    /// Walk the fan: one call per accepted destination, in edge order.
    /// The closure receives (entity, typed row, nested join handles).
    pub fn each(&mut self, ctx: &QueryCtx<'_>, mut f: impl FnMut(Id, C::Row<'_>, J::Handles)) {
        let join = &ctx.plan.joins[self.index];
        let scope = &ctx.plan.scopes[join.dest];
        let mut cache = None;

        join.fan(self.from, &ctx.binds, &ctx.params, ctx.ecs)
            .filter_map(|id| scope.bind(join.dest, id, ctx))
            .for_each(|(tgt, row, mt)| {
                let columns = get_columns::<C>(ctx.ecs, mt, &mut cache);
                // SAFETY: row is from a live record; element types were
                // checked against the plan at dispatch; aliasing was proven
                // at lowering or guarded in bind().
                f(tgt, unsafe { C::row(columns, row) }, J::handles(tgt));
            });
    }

    /// The zero-or-one fast path. Dispatch validation guarantees this
    /// join's fan shape is One (unique-shaped or pinned); the walk
    /// collapses to a single probe.
    pub fn target(&mut self, ctx: &QueryCtx<'_>) -> Option<(Id, C::Row<'_>, J::Handles)> {
        let join = &ctx.plan.joins[self.index];
        let id = join.fan(self.from, &ctx.binds, &ctx.params, ctx.ecs).next()?;
        let scope = &ctx.plan.scopes[join.dest];
        let (tgt, row, mt) = scope.bind(join.dest, id, ctx)?;
        let columns = C::get(&ctx.ecs.tables[mt.id], &mt.columns);

        // SAFETY: row is from a live record; element types were
        // checked against the plan at dispatch; aliasing was proven
        // at lowering or guarded in bind().
        Some((tgt, unsafe { C::row(columns, row) }, J::handles(tgt)))
    }
}

#[inline(always)]
fn get_columns<C: Columns>(ecs: &Ecs, matched: &MatchedTable, cache: &mut Option<(TableId, C::Get)>) -> C::Get {
    match cache {
        Some((t, c)) if *t == matched.id => *c,
        _ => {
            let cols = C::get(&ecs.tables[matched.id], &matched.columns);
            *cache = Some((matched.id, cols));
            cols
        }
    }
}
