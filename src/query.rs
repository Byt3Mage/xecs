use crate::id::Id;
use crate::storage::Storage;
use crate::table_index::TableId;
use crate::{id::Entity, storage::table::Table, world::World};
use std::collections::HashSet;
use std::vec;

//  Grammar
//
//  QUERY          ::= "SELECT" SELECT_LIST [ "WITH" WITH_LIST ]
//
//  SELECT_LIST    ::= "(" [ SELECT_ITEM { "," SELECT_ITEM } ] ")"
//
//  SELECT_ITEM    ::= COMPONENT_ACCESS | SELECT_ANYOF_GROUP
//
//  COMPONENT_ACCESS ::= [ "mut" ] COMPONENT_NAME [ "?" ]
//
//  COMPONENT_NAME ::= ["@"](IDENTIFIER | PAIR_COMPONENT)
//
//  PAIR_COMPONENT ::= "(" IDENTIFIER "," IDENTIFIER ")"
//
//  SELECT_ANYOF_GROUP ::= "(" SELECT_ANYOF_ITEM "|" SELECT_ANYOF_ITEM { "|" SELECT_ANYOF_ITEM } ")"
//
//  SELECT_ANYOF_ITEM ::= [ "mut" ] COMPONENT_NAME
//
//  WITH_LIST      ::= "(" [ WITH_ITEM { "," WITH_ITEM } ] ")"
//
//  WITH_ITEM      ::= COMPONENT_NAME | NEGATED_COMPONENT | WITH_ANYOF_GROUP | ROOT_ONLY
//
//  WITH_ANYOF_GROUP ::= "(" COMPONENT_NAME "|" COMPONENT_NAME { "|" COMPONENT_NAME } ")"
//
//  NEGATED_COMPONENT ::= "!" COMPONENT_NAME
//
//  ROOT_ONLY      ::= "-"
//
//  IDENTIFIER     ::= /[A-Za-z_][A-Za-z0-9_]*/
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum SelectAccess {
    Read,
    Write,
}

pub struct Select {
    id: Entity,
    access: SelectAccess,
}

#[derive(Debug, Clone, Copy)]
enum ColumnAccess {
    Read(usize),
    Write(usize),
}

struct Field {
    id: Entity,
    access: ColumnAccess,
    is_optional: bool,
}

impl Field {
    #[inline(always)]
    fn new(select: &Select, column_index: usize, is_optional: bool) -> Self {
        Self {
            id: select.id,
            access: match select.access {
                SelectAccess::Read => ColumnAccess::Read(column_index),
                SelectAccess::Write => ColumnAccess::Write(column_index),
            },
            is_optional,
        }
    }
}

pub struct Context<'w> {
    world: &'w World,
    fields: Vec<Field>,
}

impl<'w> Context<'w> {
    pub fn new(world: &'w World) -> Self {
        Self {
            world,
            fields: vec![],
        }
    }
}

pub struct TableView<'a> {
    table: &'a Table,
}

pub struct SelectStmt {
    /// SELECT (A, mut B)
    select: Vec<Select>,
    /// SELECT (A?, mut B?)
    optionals: Vec<Select>,
    /// SELECT ((A | mut B | C))
    anyofs: Vec<Vec<Select>>,
}

impl SelectStmt {
    pub fn new() -> Self {
        Self {
            select: vec![],
            optionals: vec![],
            anyofs: vec![],
        }
    }

    pub fn select(mut self, select: Select) -> Self {
        self.select.push(select);
        self
    }

    pub fn read(self, id: Entity) -> Self {
        self.select(Select {
            id,
            access: SelectAccess::Read,
        })
    }

    pub fn write(self, id: Entity) -> Self {
        self.select(Select {
            id,
            access: SelectAccess::Write,
        })
    }

    pub fn optional(mut self, select: Select) -> Self {
        self.optionals.push(select);
        self
    }

    pub fn select_any(mut self, any: Vec<Select>) -> Self {
        assert!(any.len() >= 2, "any_group requires at least two components");
        self.anyofs.push(any);
        self
    }
}

pub struct WithStmt {
    /// WITH (A)
    with: Vec<Entity>,
    /// WITH (!A)
    without: Vec<Entity>,
    /// WITH ((A | B))
    anyofs: Vec<Vec<Entity>>,
}

impl WithStmt {
    pub fn new() -> Self {
        Self {
            with: vec![],
            anyofs: vec![],
            without: vec![],
        }
    }

    pub fn with(mut self, id: Entity) -> Self {
        self.with.push(id);
        self
    }

    pub fn without(mut self, id: Entity) -> Self {
        self.without.push(id);
        self
    }

    pub fn with_any(mut self, any: Vec<Entity>) -> Self {
        assert!(any.len() >= 2, "any_group requires at least two components");
        self.anyofs.push(any);
        self
    }
}

pub struct QueryPlan {
    select_stmt: SelectStmt,
    with_stmt: WithStmt,
    table_ids: Vec<TableId>,
}

impl QueryPlan {
    pub fn new(select_stmt: SelectStmt, with_stmt: WithStmt) -> Self {
        Self {
            select_stmt,
            with_stmt,
            table_ids: vec![],
        }
    }

    pub fn init_tables(&mut self, world: &World) {
        let mut candidates = vec![];
        let mut has_mandatory = false;
        let map = &world.components;

        // Mandatory WITH: pick smallest
        for id in &self.with_stmt.with {
            let ci = id.map_get(map).unwrap();

            match &ci.storage {
                Storage::Tables(tables) => {
                    if !has_mandatory || tables.len() < candidates.len() {
                        candidates = tables.keys().cloned().collect()
                    }
                }
                _ => panic!("invalid storage"),
            }

            has_mandatory = true;
        }

        // Mandatory SELECT: pick smallest
        for select in &self.select_stmt.select {
            let ci = select.id.map_get(map).unwrap();
            match &ci.storage {
                Storage::Tables(tables) => {
                    if !has_mandatory || tables.len() < candidates.len() {
                        candidates = tables.keys().cloned().collect()
                    }
                }
                _ => panic!("invalid storage"),
            }
        }

        if has_mandatory {
            self.table_ids = candidates;
            return;
        }

        // No mandatory components → build from anyof groups
        let mut anyof_candidates = HashSet::new();

        // WITH anyof: union group, intersect across groups
        for group in &self.with_stmt.anyofs {
            for id in group {
                let ci = id.map_get(map).unwrap();
                match &ci.storage {
                    Storage::Tables(tables) => {
                        anyof_candidates.extend(tables.keys());
                    }
                    _ => panic!("invalid storage"),
                }
            }
        }

        // SELECT anyof: union group, intersect across groups
        for group in &self.select_stmt.anyofs {
            for select in group {
                let ci = select.id.map_get(map).unwrap();
                match &ci.storage {
                    Storage::Tables(tables) => {
                        anyof_candidates.extend(tables.keys());
                    }
                    _ => panic!("invalid storage"),
                }
            }
        }

        // Final candidate list
        self.table_ids = if anyof_candidates.is_empty() {
            world.tables.all_table_ids().copied().collect()
        } else {
            anyof_candidates.into_iter().collect()
        };
    }

    pub fn next_table<'w>(&mut self, ctx: &'w mut Context) -> Option<TableView<'w>> {
        #[inline]
        fn try_select(select: &Select, table: &Table, fields: &mut Vec<Field>) -> bool {
            if let Some(&col) = select.id.map_get(&table.column_map) {
                fields.push(Field::new(select, col, false));
                return true;
            }

            false
        }

        #[inline]
        fn try_anyof(select: &Select, table: &Table, fields: &mut Vec<Field>) -> bool {
            if let Some(&col) = select.id.map_get(&table.column_map) {
                fields.push(Field::new(select, col, true));
                return true;
            }
            false
        }

        #[inline]
        fn select_optional(select: &Select, table: &Table, fields: &mut Vec<Field>) {
            let col = select
                .id
                .map_get(&table.column_map)
                .copied()
                .unwrap_or(usize::MAX);

            fields.push(Field::new(select, col, true));
        }

        while let Some(arch_id) = self.table_ids.pop() {
            let table = &ctx.world.tables[arch_id];
            ctx.fields.clear();

            // Check with
            if !self
                .with_stmt
                .with
                .iter()
                .all(|&cid| table.signature.has_id(cid))
            {
                continue;
            }

            // Check without
            if self
                .with_stmt
                .without
                .iter()
                .any(|&cid| table.signature.has_id(cid))
            {
                continue;
            }

            // Check with anyof
            if !self
                .with_stmt
                .anyofs
                .iter()
                .all(|group| group.iter().any(|&cid| table.signature.has_id(cid)))
            {
                continue;
            }

            // Check select
            if !self
                .select_stmt
                .select
                .iter()
                .all(|comp| try_select(comp, table, &mut ctx.fields))
            {
                continue;
            }

            // Check select anyof
            if !self.select_stmt.anyofs.iter().all(|anyof| {
                anyof
                    .iter()
                    .any(|comp| try_anyof(comp, table, &mut ctx.fields))
            }) {
                continue;
            }

            // Collect optionals
            self.select_stmt
                .optionals
                .iter()
                .for_each(|comp| select_optional(comp, table, &mut ctx.fields));

            return Some(TableView { table });
        }

        None
    }
}
