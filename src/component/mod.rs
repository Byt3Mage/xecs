use std::{
    fmt::Debug,
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{
    ecs::Ecs,
    error::EcsResult,
    graph::{find_add_table, find_remove_table},
    id::{
        Id,
        allocator::{IdRecord, NotAlive},
    },
    storage::table::move_id,
    table_index::TableId,
    type_meta::TypeMeta,
    utils::ConstNonNull,
};

pub mod registry;
pub mod traits;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(transparent)]
pub struct ComponentId(u32);

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Component#{}", self.0)
    }
}

#[derive(Debug)]
pub struct StaticId<T: 'static> {
    id: u32,
    name: &'static str,
    marker: PhantomData<fn() -> T>,
}

impl<T: 'static> StaticId<T> {
    pub fn new(name: &'static str) -> Self {
        Self { id: Self::allocate(), name, marker: PhantomData }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    fn allocate() -> u32 {
        static MAX_INDEX: AtomicU32 = AtomicU32::new(0);
        MAX_INDEX.fetch_add(1, Ordering::Relaxed)
    }
}

/// A Rust type with an associated [ComponentId]
pub trait TypedStaticId: 'static + Sized {
    fn id() -> &'static StaticId<Self>;
}

#[derive(Default)]
pub struct ComponentHooks {
    pub on_add: Option<Box<dyn FnMut(Id)>>,
    pub on_remove: Option<Box<dyn FnMut(Id, ConstNonNull<u8>)>>,
    pub on_set: Option<Box<dyn FnMut(Id, NonNull<u8>)>>,
    pub default: Option<Box<dyn FnMut(NonNull<u8>)>>,
    pub clone: Option<Box<dyn FnMut(ConstNonNull<u8>, NonNull<u8>)>>,
}

pub struct ComponentInfo {
    pub(crate) name: Option<Rc<str>>,
    pub(crate) meta: Rc<TypeMeta>,
    pub(crate) tables: Vec<TableId>,
}

impl ComponentInfo {
    pub(crate) fn insert_table(&mut self, table: TableId) {
        if let Err(pos) = self.tables.binary_search(&table) {
            self.tables.insert(pos, table);
        }
    }
}

pub struct ComponentConfig {
    pub(crate) name: Option<Rc<str>>,
    pub(crate) meta: TypeMeta,
}

impl ComponentConfig {
    pub fn new() -> Self {
        Self { name: None, meta: TypeMeta::of::<()>() }
    }

    pub fn name(mut self, name: impl Into<Rc<str>>) -> Self {
        self.name = Some(name.into());
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
/// - Caller must ensure that `val` is the component data type.
pub(crate) unsafe fn insert<T: 'static>(ecs: &mut Ecs, id: Id, comp: ComponentId, val: T) -> EcsResult<Option<T>> {
    let r = ecs.ids.get(id)?;

    // SAFETY: Caller ensures the val is the component data type
    unsafe {
        let table = &ecs.tables[r.table];
        Ok(match table.col_map.get(&comp) {
            Some(&col) => Some(table.column(col).data.replace(r.row, val)),
            None => {
                let new_table = find_add_table(ecs, r.table, comp).unwrap();
                let dst_row = move_id(ecs, id, r.table, r.row, new_table);
                let nt = &ecs.tables[new_table];
                let dst_col = nt.col_map[&comp];
                nt.column(dst_col).data.write(dst_row, val);
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

pub(crate) fn has(ecs: &Ecs, r: IdRecord, comp: ComponentId) -> bool {
    ecs.tables[r.table].col_map.contains_key(&comp)
}

pub(crate) unsafe fn get<T: 'static>(ecs: &Ecs, r: IdRecord, comp: ComponentId) -> Option<&T> {
    unsafe { ecs.tables[r.table].get(comp, r.row) }
}

pub(crate) unsafe fn get_mut<T: 'static>(ecs: &Ecs, r: IdRecord, comp: ComponentId) -> Option<&mut T> {
    unsafe { ecs.tables[r.table].get_mut(comp, r.row) }
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
