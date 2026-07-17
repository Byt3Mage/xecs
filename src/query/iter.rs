use std::{cell::Cell, marker::PhantomData};

use crate::{
    Id,
    query::{
        QueryCtx,
        join::Join,
        logical::ScopeId,
        physical::{MatchedTable, PhysicalScope},
    },
    storage::table::Table,
};

pub(crate) struct Bindings(Box<[Cell<Id>]>);

impl Bindings {
    /// One slot per tracked scope. Initial value is never read: a slot
    /// is only readable via probes/guards on LATER scopes, and lowering
    /// guarantees the tracked scope binds (and writes) first.
    pub(crate) fn new(count: usize) -> Self {
        Self(vec![Cell::new(Id::NULL); count].into_boxed_slice())
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

pub(crate) struct Params(Box<[Id]>);

impl Params {
    pub(crate) fn get(&self, index: usize) -> Id {
        self.0[index]
    }
}

pub trait Columns {
    type Get: Copy;
    type Row<'a>;

    fn get(table: &Table, column_indices: &[usize]) -> Self::Get;
    unsafe fn row<'a>(columns: Self::Get, row: u32) -> Self::Row<'a>;
}

pub trait Joins {
    type Handles;
    fn handles(from: Id) -> Self::Handles;
}

/// Leaf scopes: no nested joins.
impl Joins for () {
    type Handles = ();
    #[inline(always)]
    fn handles<'w>(_: Id) {}
}

/// Type-level join spec used in the query's signature.
/// Never constructed: N selects plan.joins[N], C/J describe the dest
/// scope. The runtime value received is [Join].
pub struct TJoin<const N: usize, F, J = ()>(PhantomData<(F, J)>);

impl<const N: usize, C: Columns, J: Joins> Joins for TJoin<N, C, J> {
    type Handles = (Join<C, J>,);

    #[inline(always)]
    fn handles(from: Id) -> Self::Handles {
        (Join::new(N, from),)
    }
}

macro_rules! impl_join_tuple {
    ($(($n:ident, $f:ident, $j:ident)),+) => {
        impl<$(const $n: usize, $f: Columns, $j: Joins),+> Joins
            for ($(TJoin<$n, $f, $j>,)+)
        {
            type Handles = ($(Join<$f, $j>,)+);

            #[inline(always)]
            fn handles( from: Id ) -> Self::Handles {
                ($(Join::new($n, from),)+)
            }
        }
    };
}
impl_join_tuple!((N0, C0, J0), (N1, C1, J1));
impl_join_tuple!((N0, C0, J0), (N1, C1, J1), (N2, C2, J2));

impl MatchedTable {
    #[inline]
    pub fn each_row<C: Columns>(&self, ctx: &QueryCtx<'_>, mut f: impl FnMut(C::Row<'_>)) {
        let table = &ctx.ecs.tables[self.id];
        let num_rows = table.num_rows();

        if num_rows != 0 {
            let columns = C::get(table, &self.columns);

            for row in 0..num_rows {
                f(unsafe { C::row(columns, row) });
            }
        }
    }
}
