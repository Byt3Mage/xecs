use std::{rc::Rc, sync::Arc};

use crate::{
    component::id::ComponentId,
    ecs::Ecs,
    graph::{find_add_table, find_remove_table},
    id::{
        Id,
        allocator::{IdRecord, NotAlive},
    },
    storage::table::move_id,
    table_index::TableId,
    type_meta::TypeMeta,
};

pub mod id;
pub mod registry;
pub mod traits;

#[derive(Default)]
pub struct ComponentHooks {
    pub on_insert: Option<Box<dyn FnMut(Id, *mut u8)>>,
    pub on_remove: Option<Box<dyn FnMut(Id, *mut u8)>>,
    pub clone: Option<Box<dyn FnMut(*const u8, *mut u8)>>,
    pub default: Option<Box<dyn FnMut(*mut u8)>>,
}

pub type Path = Arc<str>;

pub struct ComponentInfo {
    pub(crate) name: Option<Path>,
    pub(crate) meta: TypeMeta,
    pub(crate) tables: Vec<TableId>,
}

impl ComponentInfo {
    pub(crate) fn insert_table(&mut self, table: TableId) {
        if let Err(pos) = self.tables.binary_search(&table) {
            self.tables.insert(pos, table);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComponentConfig<T: Into<Path>> {
    pub(crate) name: Option<T>,
    pub(crate) meta: TypeMeta,
}

impl<T: Into<Arc<str>>> ComponentConfig<T> {
    pub fn new() -> Self {
        Self { name: None, meta: TypeMeta::of::<()>() }
    }

    pub fn name(mut self, name: T) -> Self {
        self.name = Some(name);
        self
    }

    pub fn meta(mut self, meta: TypeMeta) -> Self {
        self.meta = meta;
        self
    }
}

/// Inserts the value of a component for an id.
/// Returns the previously held value, if any.
///
/// # Safety
/// - Caller must ensure that `T` is the component data type.
pub(crate) unsafe fn insert<T: 'static>(
    ecs: &mut Ecs,
    id: Id,
    comp: ComponentId,
    val: T,
) -> Result<Option<T>, NotAlive> {
    let r = ecs.ids.get(id)?;

    // SAFETY: Caller ensures the val is the component data type
    unsafe {
        let table = &ecs.tables[r.table];
        Ok(match table.col_map.get(&comp) {
            Some(&idx) => {
                let col = table.column(idx);
                let row = col.data.row_ptr(r.row);
                Some(row.cast().replace(val))
            }
            None => {
                let dst_table_id = find_add_table(ecs, r.table, comp).unwrap();
                let dst_row = move_id(ecs, id, r.table, r.row, dst_table_id);
                let dst_table = &ecs.tables[dst_table_id];
                let dst_col = dst_table.col_map[&comp];
                let dst_row = dst_table.column(dst_col).data.row_ptr(dst_row);
                dst_row.cast().write(val);
                None
            }
        })
    }
}

pub(crate) unsafe fn remove(ecs: &mut Ecs, id: Id, comp: ComponentId) -> Result<(), NotAlive> {
    let r = ecs.ids.get(id)?;
    if ecs.components.get(comp).tables.contains(&r.table) {
        let dst_table = find_remove_table(ecs, r.table, comp).unwrap();
        unsafe { move_id(ecs, id, r.table, r.row, dst_table) };
    }
    Ok(())
}

#[inline(always)]
pub(crate) fn has(ecs: &Ecs, r: IdRecord, comp: ComponentId) -> bool {
    ecs.tables[r.table].col_map.contains_key(&comp)
}

pub(crate) unsafe fn get<T: 'static>(ecs: &Ecs, r: IdRecord, comp: ComponentId) -> Option<&T> {
    let table = &ecs.tables[r.table];
    let col = table.col_map.get(&comp).map(|&i| table.column(i))?;
    Some(unsafe { col.data.row_ptr(r.row).cast().as_ref() })
}

pub(crate) unsafe fn get_mut<T: 'static>(ecs: &mut Ecs, r: IdRecord, comp: ComponentId) -> Option<&mut T> {
    let table = &ecs.tables[r.table];
    let col = table.col_map.get(&comp).map(|&i| table.column(i))?;
    Some(unsafe { col.data.row_ptr(r.row).cast().as_mut() })
}

/// Sorted list of component ids in a [Table](crate::storage::table::Table)
#[derive(Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct Signature(Rc<[ComponentId]>);

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Clone for Signature {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl From<Vec<ComponentId>> for Signature {
    fn from(mut value: Vec<ComponentId>) -> Self {
        Self({
            value.sort();
            value.dedup();
            value.into()
        })
    }
}

impl<const N: usize> From<[ComponentId; N]> for Signature {
    fn from(value: [ComponentId; N]) -> Self {
        Vec::from(value).into()
    }
}

impl std::ops::Deref for Signature {
    type Target = [ComponentId];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Signature {
    #[inline]
    pub fn ids(&self) -> &[ComponentId] {
        &self.0
    }

    #[inline]
    pub fn find_id(&self, id: &ComponentId) -> Option<usize> {
        self.binary_search(id).ok()
    }

    #[inline]
    pub fn has_id(&self, id: &ComponentId) -> bool {
        self.binary_search(id).is_ok()
    }

    /// Creates a new sorted list from [Signature] and `with`
    ///
    /// Returns `None` if self already contains `with`.
    pub fn try_extend(&self, with: ComponentId) -> Option<Self> {
        match self.binary_search(&with) {
            Ok(_) => None,
            Err(pos) => Some({
                let mut new_sig = Vec::with_capacity(pos);
                new_sig.extend_from_slice(&self[..pos]);
                new_sig.push(with);
                new_sig.extend_from_slice(&self[pos..]);
                new_sig.into()
            }),
        }
    }

    /// Creates a new sorted list from [Signature] without `from`.
    ///
    /// Returns `None` if self doesn't contain `from`.
    pub fn try_shrink(&self, from: ComponentId) -> Option<Self> {
        match self.binary_search(&from) {
            Ok(pos) => Some({
                let mut new_list = Vec::from(self.as_ref());
                new_list.remove(pos);
                new_list.into()
            }),
            Err(_) => None,
        }
    }
}
