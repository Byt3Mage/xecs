use std::{
    fmt::Debug,
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
    sync::atomic::{AtomicU32, Ordering},
};

use ahash::AHashSet;
use smallvec::{SmallVec, smallvec};

use crate::{
    access::{AccessType, StaticAccess},
    component::private::SealedGetMulti,
    ecs::Ecs,
    error::{EcsResult, Error, IdNotComponent},
    graph::{find_add_table, find_remove_table},
    id::{Id, manager::IdRecord, map::IdMap},
    storage::{Storage, StorageType, resource::Resource, sparse::SparseSet, table::move_id},
    type_meta::TypeMeta,
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

pub(crate) struct Component {
    pub(crate) meta: Rc<TypeMeta>,
    pub(crate) storage: Storage,
    pub(crate) resource: Option<Resource>,
}

impl Drop for Component {
    fn drop(&mut self) {
        if let Some(res) = &mut self.resource {
            res.destroy(&self.meta);
        }
    }
}
pub struct ComponentBuilder<T: 'static> {
    pub(crate) name: Option<Rc<str>>,
    pub(crate) storage: StorageType,
    pub(crate) resource: Option<T>,
}

impl<T: 'static> ComponentBuilder<T> {
    pub fn new() -> Self {
        Self {
            name: None,
            storage: StorageType::default(),
            resource: None,
        }
    }

    pub fn name(mut self, name: impl Into<Rc<str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[inline]
    pub fn storage(mut self, storage_type: StorageType) -> Self {
        self.storage = storage_type;
        self
    }

    pub fn resource(mut self, value: Option<T>) -> Self {
        self.resource = value;
        self
    }

    pub(crate) fn build(self, components: &mut IdMap<Component>, id: Id) {
        let meta = Rc::new(TypeMeta::of::<T>());
        let resource = self.resource.map(Resource::new);
        let storage = match self.storage {
            StorageType::Tables => Storage::Tables(AHashSet::new()),
            StorageType::Sparse => Storage::Sparse(SparseSet::new(id, meta.clone())),
        };

        components.insert(id, Component { meta, storage, resource });
    }
}

/// Inserts the value of a component for an id.
/// Returns the previously held value, if any.
///
/// # Safety
/// - Caller must ensure that `val` is the component data type.
pub(crate) unsafe fn insert<T: 'static>(ecs: &mut Ecs, id: Id, comp: Id, val: T) -> EcsResult<Option<T>> {
    let r = ecs.ids.get(id)?;
    let ci = ecs.components.get_mut(comp).ok_or(IdNotComponent(comp))?;

    // SAFETY: Caller ensures the val is the component data type
    unsafe {
        match &mut ci.storage {
            Storage::Sparse(set) => Ok(set.insert(id, val)),
            Storage::Tables(_) => {
                let table = &ecs.tables[r.table];
                if let Some(&col) = table.col_map.get(comp) {
                    // Component already present: replace in place.
                    Ok(Some(table.column(col).replace(r.row, val)))
                } else {
                    // Component absent: move entity to the table that has it.
                    let new_table = find_add_table(ecs, r.table, comp).unwrap();
                    let dst_row = move_id(ecs, id, r.table, r.row, new_table);
                    let nt = &ecs.tables[new_table];
                    let dst_col = nt.col_map[comp];
                    nt.column(dst_col).write(dst_row, val);
                    Ok(None)
                }
            }
        }
    }
}

pub(crate) unsafe fn remove(ecs: &mut Ecs, id: Id, tag: Id) -> EcsResult<()> {
    let r = ecs.ids.get(id)?;
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
    let r = ecs.ids.get(id)?;
    let cm = ecs.components.get(comp).ok_or(IdNotComponent(comp))?;
    Ok(match &cm.storage {
        Storage::Sparse(set) => set.contains(id),
        Storage::Tables(tables) => tables.contains(&r.table),
    })
}

pub(crate) unsafe fn get<T: 'static>(ecs: &Ecs, id: Id, r: IdRecord, comp: Id) -> EcsResult<Option<&T>> {
    let ci = ecs.components.get(comp).ok_or(IdNotComponent(comp))?;
    Ok(unsafe {
        match &ci.storage {
            Storage::Tables(_) => ecs.tables[r.table].get(comp, r.row),
            Storage::Sparse(set) => set.get(id),
        }
    })
}

pub(crate) unsafe fn get_mut<T: 'static>(ecs: &Ecs, id: Id, r: IdRecord, comp: Id) -> EcsResult<Option<&mut T>> {
    let cm = ecs.components.get(comp).ok_or(IdNotComponent(comp))?;
    Ok(unsafe {
        match &cm.storage {
            Storage::Tables(_) => ecs.tables[r.table].get_mut(comp, r.row),
            Storage::Sparse(set) => set.get_mut(id),
        }
    })
}

pub(crate) unsafe fn insert_resource<T: 'static>(ecs: &mut Ecs, id: Id, value: T) -> EcsResult<Option<T>> {
    let cm = ecs.components.get_mut(id).ok_or(IdNotComponent(id))?;
    let prev = match &mut cm.resource {
        Some(r) => Some(unsafe { r.replace(value) }),
        None => {
            cm.resource = Some(Resource::new(value));
            None
        }
    };
    Ok(prev)
}

pub(crate) unsafe fn resource<T: 'static>(ecs: &Ecs, id: Id) -> EcsResult<&T> {
    let cm = ecs.components.get(id).ok_or(IdNotComponent(id))?;
    match &cm.resource {
        Some(r) => Ok(unsafe { r.get() }),
        None => Err(Error::MissingResource { id }),
    }
}

pub(crate) unsafe fn resource_mut<T: 'static>(ecs: &Ecs, id: Id) -> EcsResult<&mut T> {
    let cm = ecs.components.get(id).ok_or(IdNotComponent(id))?;
    match &cm.resource {
        Some(r) => Ok(unsafe { r.get_mut() }),
        None => Err(Error::MissingResource { id }),
    }
}

mod private {
    pub trait SealedAccess {}
    pub trait SealedGetMulti {}
}

use private::SealedAccess;

pub trait ComponentAccess: Sized + private::SealedAccess {
    type RemoveRef: 'static;
    type Get<'a>;
    const ACCESS: AccessType;

    /// # Safety
    /// Caller guarantees this access does not alias another live
    /// borrow of the same component for `id` (single access, or validated
    /// disjoint within a tuple).
    unsafe fn fetch<'a>(ecs: &'a Ecs, id: Id, r: IdRecord, comp: Id) -> EcsResult<Self::Get<'a>>;
}

impl<T: 'static> SealedAccess for &T {}
impl<T: 'static> SealedAccess for &mut T {}

impl<T: 'static> ComponentAccess for &T {
    type RemoveRef = T;
    type Get<'a> = &'a T;
    const ACCESS: AccessType = AccessType::Read;

    unsafe fn fetch<'a>(ecs: &'a Ecs, id: Id, r: IdRecord, comp: Id) -> EcsResult<&'a T> {
        unsafe { get::<T>(ecs, id, r, comp)?.ok_or(Error::MissingComponent { id, comp }) }
    }
}

impl<T: 'static> ComponentAccess for &mut T {
    type RemoveRef = T;
    type Get<'a> = &'a mut T;
    const ACCESS: AccessType = AccessType::Write;

    unsafe fn fetch<'a>(ecs: &'a Ecs, id: Id, r: IdRecord, comp: Id) -> EcsResult<&'a mut T> {
        unsafe { get_mut::<T>(ecs, id, r, comp)?.ok_or(Error::MissingComponent { id, comp }) }
    }
}

impl<T: ComponentAccess> SealedGetMulti for T {}

pub trait GetMulti: Sized + private::SealedGetMulti {
    type Output<'a>;
    fn accesses() -> SmallVec<[StaticAccess; 8]>;
    fn create(ecs: &mut Ecs, id: Id) -> EcsResult<Self::Output<'_>>;
}

impl<T: ComponentAccess> GetMulti for T
where
    T::RemoveRef: TypedStaticId,
{
    type Output<'a> = T::Get<'a>;

    fn accesses() -> SmallVec<[StaticAccess; 8]> {
        smallvec![StaticAccess { id: T::RemoveRef::id().id, ty: T::ACCESS }]
    }

    fn create(ecs: &mut Ecs, id: Id) -> EcsResult<Self::Output<'_>> {
        let r = ecs.ids.get(id)?;
        let comp = ecs.id_t::<T::RemoveRef>()?;
        // SAFETY: single access; &mut Ecs guarantees uniqueness. The cast from
        // the &mut-Ecs borrow to T::Get (& or &mut) is sound because there is
        // exactly one access and we hold the unique world reference.
        unsafe { T::fetch(ecs, id, r, comp) }
    }
}

macro_rules! impl_tuple_params {
    ($($T:ident),*) => {
        impl<$($T: ComponentAccess),*> private::SealedGetMulti for ($($T,) *) {}
        impl<$($T: ComponentAccess),*> GetMulti for ($($T,) *)
        where
            $($T::RemoveRef: TypedStaticId,)*
        {
            type Output<'a> = ($($T::Get<'a>,)*);


            fn accesses() -> SmallVec<[StaticAccess; 8]> {
                smallvec![$(StaticAccess { id: $T::RemoveRef::id().id, ty: $T::ACCESS }),*]
            }

            fn create(ecs: &mut Ecs, id: Id) -> EcsResult<Self::Output<'_>> {
                let r = ecs.ids.get(id)?;

                // Validate internal disjointness: no &mut aliases another access
                // of the same component. Panics (or Err) per your convention.
                crate::validate::check_multi_get::<($($T,) *)>();

                // SAFETY: check_multi_get proved no aliasing among the tuple's
                // accesses; &mut Ecs proves no external aliasing. Each fetch is
                // therefore the unique live borrow of its component.
                let ecs: &Ecs = ecs;
                unsafe { Ok(($($T::fetch(ecs, id, r, ecs.id_t::<$T::RemoveRef>()?)?,)*)) }
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
