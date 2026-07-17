use crate::{
    Ecs, Id,
    component::ComponentId,
    query::{
        iter::{Bindings, Columns, Joins, Params},
        logical::{Access, Join, LogicalPlan, RelCheck, RelTarget, Relation, Scope, ScopeId},
        physical::PhysicalPlan,
    },
};

mod error;
mod fetch;
mod iter;
mod join;
mod logical;
mod physical;

pub struct QueryBuilder {
    plan: LogicalPlan,
    physical: PhysicalPlan,
    /// Visible labels: (name, scope). Truncated when a scope closes,
    /// so only ancestor labels are ever resolvable.
    labels: Vec<(Box<str>, ScopeId)>,
    /// Scope currently being described. Joins temporarily redirect this.
    curr: ScopeId,
}

impl QueryBuilder {
    fn access(&mut self, access: Access) -> &mut Self {
        self.plan.scopes[self.curr].access.push(access);
        self
    }

    pub fn with(&mut self, c: ComponentId) -> &mut Self {
        self.plan.scopes[self.curr].with.push(c);
        self
    }

    pub fn without(&mut self, c: ComponentId) -> &mut Self {
        self.plan.scopes[self.curr].without.push(c);
        self
    }

    fn rel_filter(&mut self, filter: RelCheck) -> &mut Self {
        if let RelTarget::Label(s) = filter.relation.target {
            assert!(s != self.curr, "self-unification is meaningless");
        }
        self.plan.scopes[self.curr].rel_check.push(filter);
        self
    }

    /// AS name, labels the scope being described.
    pub fn label(&mut self, name: &str) -> &mut Self {
        assert!(
            self.labels.iter().all(|(n, _)| n.as_ref() != name),
            "duplicate label `{name}`"
        );
        self.labels.push((name.into(), self.curr));
        self
    }

    fn resolve_label(&self, name: &str) -> ScopeId {
        self.labels
            .iter()
            .find_map(|(n, s)| (n.as_ref() == name).then_some(*s))
            .unwrap_or_else(|| panic!("label `{name}` not visible here"))
    }

    fn join(&mut self, relation: Relation, optional: bool, f: impl FnOnce(&mut Self)) -> &mut Self {
        let source = self.curr;
        let target = self.plan.scopes.len();

        // Appended BEFORE descending: joins end up in declaration order,
        // so every join's `from` scope is bound before it executes.
        self.plan.scopes.push(Scope::default());
        self.plan
            .joins
            .push(Join { relation, optional, from: source, dest: target });

        let (current, labels) = (self.curr, self.labels.len());
        self.curr = target;
        f(self);
        self.curr = current;
        self.labels.truncate(labels);
        self
    }

    pub fn build(self) -> LogicalPlan {
        self.plan
    }

    pub fn each_row<C: Columns>(&mut self, ecs: &mut Ecs, params: Params, mut f: impl FnMut(Id, C::Row<'_>)) {
        let driver = &self.physical.scopes[0];
        let ctx = QueryCtx {
            ecs,
            plan: &self.physical,
            binds: Bindings::new(self.physical.scopes.len()),
            params,
        };

        driver.tables.iter().for_each(|mt| {
            let table = &ctx.ecs.tables[mt.id];
            let num_rows = table.num_rows();

            if num_rows != 0 {
                return;
            }

            let columns = C::get(table, &mt.columns);
            let ids = table.ids();

            for row in 0..num_rows {
                let id = ids[row as usize];

                if !driver.checks.iter().all(|c| c.eval(id, &ctx)) {
                    continue;
                }

                f(id, unsafe { C::row(columns, row) });
            }
        });
    }

    pub fn each<C: Columns, J: Joins>(
        &mut self,
        ecs: &mut Ecs,
        params: Params,
        mut f: impl FnMut(Id, C::Row<'_>, J::Handles),
    ) {
        let driver = &self.physical.scopes[0];
        let ctx = QueryCtx {
            ecs,
            plan: &self.physical,
            binds: Bindings::new(self.physical.scopes.len()),
            params,
        };

        driver.tables.iter().for_each(|mt| {
            let table = &ctx.ecs.tables[mt.id];
            let num_rows = table.num_rows();

            if num_rows != 0 {
                return;
            }

            let columns = C::get(table, &mt.columns);
            let ids = table.ids();

            for row in 0..num_rows {
                let id = ids[row as usize];

                if !driver.checks.iter().all(|c| c.eval(id, &ctx)) {
                    continue;
                }

                ctx.binds.set(0, id);

                f(id, unsafe { C::row(columns, row) }, J::handles(id));
            }
        });
    }
}

pub struct QueryCtx<'w> {
    pub(crate) ecs: &'w Ecs,
    pub(crate) plan: &'w PhysicalPlan,
    pub(crate) binds: Bindings,
    pub(crate) params: Params,
}
