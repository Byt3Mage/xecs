use std::{
    fmt::Debug,
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
    sync::atomic::{AtomicU32, Ordering},
};

use ahash::AHashSet;

use crate::{
    ecs::Ecs,
    error::{EcsResult, Error, IdNotComponent, InvalidId},
    graph::{find_add_table, find_remove_table},
    id::{Id, manager::IdRecord, map::IdMap},
    storage::{Storage, StorageType, singleton::Singleton, sparse::SparseSet, table::move_id},
    type_meta::TypeMeta,
    unsafe_ecs::UnsafeEcsCell,
    utils::ConstNonNull,
};

#[derive(Debug)]
pub struct StaticId<T: 'static> {
    id: u32,
    name: &'static str,
    storage: StorageType,
    marker: PhantomData<fn() -> T>,
}

impl<T: 'static> StaticId<T> {
    pub fn new(name: &'static str, storage: StorageType) -> Self {
        Self {
            id: Self::allocate(),
            name,
            storage,
            marker: PhantomData,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn storage(&self) -> StorageType {
        self.storage
    }

    fn allocate() -> u32 {
        static MAX_INDEX: AtomicU32 = AtomicU32::new(0);
        MAX_INDEX.fetch_add(1, Ordering::Relaxed)
    }
}

/// A Rust type with an associated ECS [Id].
pub trait TypedStaticId: 'static + Sized {
    fn id() -> &'static StaticId<Self>;
}

pub struct ComponentHooks {
    pub on_add: Option<Box<dyn FnMut(Id)>>,
    pub on_remove: Option<Box<dyn FnMut(Id, ConstNonNull<u8>)>>,
    pub on_set: Option<Box<dyn FnMut(Id, NonNull<u8>)>>,
    pub default: Option<Box<dyn FnMut(NonNull<u8>)>>,
    pub clone: Option<Box<dyn FnMut(ConstNonNull<u8>, NonNull<u8>)>>,
}

pub(crate) struct ComponentMeta {
    pub(crate) storage: Storage,
    pub(crate) type_meta: Rc<TypeMeta>,
    pub(crate) singleton: Option<Singleton>,
}

pub struct ComponentBuilder<T: 'static> {
    pub(crate) name: Option<Rc<str>>,
    pub(crate) storage_type: StorageType,
    pub(crate) _marker: PhantomData<fn() -> T>,
}

impl<T: 'static> ComponentBuilder<T> {
    pub fn new() -> Self {
        Self {
            name: None,
            storage_type: StorageType::default(),
            _marker: PhantomData,
        }
    }

    pub fn name(mut self, name: impl Into<Rc<str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[inline]
    pub fn storage(mut self, storage_type: StorageType) -> Self {
        self.storage_type = storage_type;
        self
    }

    pub(crate) fn build(self, components: &mut IdMap<ComponentMeta>, id: Id) {
        let type_meta = Rc::new(TypeMeta::of::<T>());
        let storage = match self.storage_type {
            StorageType::Tables => Storage::Tables(AHashSet::new()),
            StorageType::Sparse => Storage::Sparse(SparseSet::new(id, type_meta.clone())),
        };

        components.insert(id, ComponentMeta { type_meta, storage, singleton: None });
    }
}

/// Inserts the value of a component for an id.
/// Returns the previously held value, if any.
///
/// # Safety
/// - Caller must ensure that `val` is the component data type.
pub(crate) unsafe fn insert<T: 'static>(ecs: &mut Ecs, id: Id, comp: Id, val: T) -> EcsResult<Option<T>> {
    let r = ecs.ids.get(id).ok_or(InvalidId(id))?;
    let ci = ecs.components.get_mut(comp).ok_or(IdNotComponent(comp))?;

    // SAFETY: Caller ensures the val is the component data type
    unsafe {
        match &mut ci.storage {
            Storage::Sparse(set) => Ok(set.insert(id, val)),
            Storage::Tables(_) => {
                let table = &ecs.tables[r.table];

                let prev = if let Some(col) = table.col_map.get(comp) {
                    Some(table.data.columns[*col].replace(r.row, val))
                } else {
                    // Move id to new table
                    let new_table = find_add_table(ecs, r.table, comp).unwrap();
                    let dst_row = move_id(ecs, id, r.table, r.row, new_table);

                    // Write data into new column
                    let new_table = &ecs.tables[new_table];
                    let dst_col = new_table.col_map[comp];
                    new_table.data.columns[dst_col].write(dst_row, val);
                    None
                };

                Ok(prev)
            }
        }
    }
}

pub(crate) unsafe fn remove(ecs: &mut Ecs, id: Id, tag: Id) -> EcsResult<()> {
    let r = ecs.ids.get(id).ok_or(InvalidId(id))?;
    let cm = ecs.components.get_mut(tag).ok_or(IdNotComponent(tag))?;

    unsafe {
        match &mut cm.storage {
            Storage::Sparse(set) => set.remove(id),
            Storage::Tables(tables) => {
                if tables.contains(&r.table) {
                    let dst_table = find_remove_table(ecs, r.table, tag).unwrap();
                    move_id(ecs, id, r.table, r.row, dst_table);
                }
            }
        }
    };

    Ok(())
}

pub(crate) fn has(ecs: &Ecs, id: Id, comp: Id) -> EcsResult<bool> {
    let r = ecs.ids.get(id).ok_or(InvalidId(id))?;
    let cm = ecs.components.get(comp).ok_or(IdNotComponent(comp))?;
    Ok(match &cm.storage {
        Storage::Sparse(set) => set.contains(id),
        Storage::Tables(tables) => tables.contains(&r.table),
    })
}

pub(crate) unsafe fn get<T: 'static>(ecs: &Ecs, id: Id, r: IdRecord, comp: Id) -> EcsResult<&T> {
    let ci = ecs.components.get(comp).ok_or(IdNotComponent(comp))?;

    let res = unsafe {
        match &ci.storage {
            Storage::Tables(_) => ecs.tables[r.table].get(comp, r.row),
            Storage::Sparse(set) => set.get(id),
        }
    };

    res.ok_or(Error::MissingComponent { id, comp })
}

pub(crate) unsafe fn get_mut<T: 'static>(ecs: &mut Ecs, id: Id, r: IdRecord, comp: Id) -> EcsResult<&mut T> {
    let cm = ecs.components.get_mut(comp).ok_or(IdNotComponent(comp))?;
    let res = unsafe {
        match &mut cm.storage {
            Storage::Tables(_) => ecs.tables[r.table].get_mut(comp, r.row),
            Storage::Sparse(set) => set.get_mut(id),
        }
    };

    res.ok_or(Error::MissingComponent { id, comp })
}

pub(crate) mod private {
    pub trait Sealed {}
}

pub trait Param: private::Sealed {
    type Output<'a>;
    const IS_IMMUTABLE: bool;

    /// # Safety
    /// Caller must ensure access doesn't violate aliasing rules.
    unsafe fn make(ecs: UnsafeEcsCell<'_>, id: Id, r: IdRecord) -> EcsResult<Self::Output<'_>>;
}

impl<T: Param> private::Sealed for T {}
impl<T: TypedStaticId> Param for &T {
    type Output<'a> = &'a T;
    const IS_IMMUTABLE: bool = true;

    unsafe fn make(cell: UnsafeEcsCell<'_>, id: Id, r: IdRecord) -> EcsResult<Self::Output<'_>> {
        unsafe {
            let ecs = cell.ecs();
            get(ecs, id, r, ecs.id_t::<T>()?)
        }
    }
}

impl<T: TypedStaticId> Param for &mut T {
    type Output<'a> = &'a mut T;
    const IS_IMMUTABLE: bool = false;

    unsafe fn make(cell: UnsafeEcsCell<'_>, id: Id, r: IdRecord) -> EcsResult<Self::Output<'_>> {
        unsafe {
            let ecs = cell.world_mut();
            get_mut(ecs, id, r, ecs.id_t::<T>()?)
        }
    }
}

pub trait Params: Sized + private::Sealed {
    type ParamsType<'a>;
    const ALL_IMMUTABLE: bool;

    /// # Safety
    /// Caller must ensure access validation has been performed.
    unsafe fn create(cell: UnsafeEcsCell<'_>, id: Id) -> EcsResult<Self::ParamsType<'_>>;
}

impl<T: Param> Params for T {
    type ParamsType<'a> = T::Output<'a>;
    const ALL_IMMUTABLE: bool = T::IS_IMMUTABLE;

    unsafe fn create(cell: UnsafeEcsCell<'_>, id: Id) -> EcsResult<Self::ParamsType<'_>> {
        unsafe {
            let r = cell.ecs().ids.get(id).ok_or(InvalidId(id))?;
            T::make(cell, id, r)
        }
    }
}

pub(crate) fn insert_singleton<T: 'static>(ecs: &mut Ecs, id: Id, val: T) -> EcsResult<Option<T>> {
    let cm = ecs.components.get_mut(id).ok_or(IdNotComponent(id))?;
    let prev = match &mut cm.singleton {
        Some(s) => {
            let mut singleton = s.borrow_mut::<T>();
            Some(std::mem::replace(&mut *singleton, val))
        }
        None => {
            cm.singleton = Some(Singleton::new(id, cm.type_meta.clone(), val));
            None
        }
    };

    Ok(prev)
}

macro_rules! impl_tuple_params {
    ($($t:ident),*) => {
        impl<$($t: Param),*> private::Sealed for ($($t,) *) {}
        impl<$($t: Param),*> Params for ($($t,) *) {
            type ParamsType<'a> = ($($t::Output<'a>,)*);
            const ALL_IMMUTABLE: bool = { $($t::IS_IMMUTABLE &&)* true };

            unsafe fn create(cell: UnsafeEcsCell<'_>, id: Id) -> EcsResult<Self::ParamsType<'_>> {
                unsafe {
                    let r = cell.ecs().ids.get(id).ok_or(InvalidId(id))?;
                    Ok(($($t::make(cell, id, r)?,)*))
                }
            }
        }
    }
}

impl_tuple_params!(P0);
impl_tuple_params!(P0, P1);
impl_tuple_params!(P0, P1, P2);
impl_tuple_params!(P0, P1, P2, P3);
impl_tuple_params!(P0, P1, P2, P3, P4);
impl_tuple_params!(P0, P1, P2, P3, P4, P5);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7, P8);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12);
