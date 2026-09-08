use std::sync::Arc;

use crate::{
    ComponentKey,
    component::id::ComponentId,
    ecs::Ecs,
    id::allocator::IdRecord,
    key::{UnregisteredKey, unregistered},
    table_registry::TableId,
    type_meta::{HasMeta, TypeMeta},
};

pub mod id;
pub mod registry;
pub mod signature;

pub type Path = Arc<str>;

/// Reserved: `C#<n>` addresses a component by id in string queries,
/// and is the display form of an unnamed component.
pub const ID_PREFIX: &str = "C#";

#[derive(Debug, thiserror::Error)]
pub enum ComponentRegisterError {
    #[error("component path `{0}` is already registered")]
    DuplicatePath(Path),
    #[error("component path `{0}` uses the reserved `{ID_PREFIX}` prefix")]
    ReservedPrefix(Path),
}

#[derive(Debug, Clone)]
pub struct ComponentConfig {
    pub path: Option<Path>,
    pub meta: TypeMeta,
}

impl ComponentConfig {
    pub fn new() -> Self {
        Self { path: None, meta: TypeMeta::of::<()>() }
    }

    pub fn named(path: impl Into<Path>) -> Self {
        Self {
            path: Some(path.into()),
            meta: TypeMeta::of::<()>(),
        }
    }

    pub fn path(mut self, path: impl Into<Path>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn meta(mut self, meta: TypeMeta) -> Self {
        self.meta = meta;
        self
    }
}

pub struct ComponentInfo {
    pub path: Path,
    pub meta: TypeMeta,
    pub(crate) tables: Vec<TableId>,
}

impl ComponentInfo {
    pub(crate) fn insert_table(&mut self, table: TableId) {
        if let Err(pos) = self.tables.binary_search(&table) {
            self.tables.insert(pos, table);
        }
    }
}

/// Resolve `key` to the id it registered under.
///
/// This is the sole bridge between a typed key and an untyped column,
/// and the sole justification for the `*_raw` calls below: the slot →
/// id mapping is written once, at registration, from `T::META` of this
/// same key type. A `ComponentId` obtained any other way (`new_component`,
/// `find_by_name`) cannot reach these functions, because they take a key.
#[inline(always)]
pub fn resolve<T>(ecs: &Ecs, key: ComponentKey<T>) -> Result<ComponentId, UnregisteredKey>
where
    T: HasMeta,
{
    ecs.components.find(key).ok_or_else(|| unregistered(key.untyped()))
}

pub(crate) unsafe fn get<T>(ecs: &Ecs, r: IdRecord, comp: ComponentId) -> Option<&T> {
    let table = &ecs.tables[r.table];
    let column = table.col_map.try_get(comp).map(|i| table.column(i))?;
    Some(unsafe { column.row_ptr(r.row).cast().as_ref() })
}

pub(crate) unsafe fn get_mut<T>(ecs: &mut Ecs, r: IdRecord, comp: ComponentId) -> Option<&mut T> {
    let table = &ecs.tables[r.table];
    let column = table.col_map.try_get(comp).map(|i| table.column(i))?;
    Some(unsafe { column.row_ptr(r.row).cast().as_mut() })
}
