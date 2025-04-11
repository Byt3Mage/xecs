use const_assert::const_assert;
use std::{alloc::Layout, collections::{hash_map::Entry, HashMap}, marker::PhantomData, ops::Deref, ptr::NonNull, rc::Rc};
use crate::{component::ComponentValue, entity::Entity, id::Id, pointer::ConstNonNull};

/// Built-in move function for components.
/// 
/// # Safety
/// - The caller must ensure that the pointers are non-null and aligned for T.
/// - The caller must ensure to never use src after the move.
unsafe fn move_fn<T>(src: NonNull<u8>, dst: NonNull<u8>) {
    let src = src.as_ptr().cast::<T>();
    let dst = dst.as_ptr().cast::<T>();
    unsafe { dst.write(src.read()) };
}

/// Built-in drop function for components.
/// 
/// # Safety
/// - The caller must ensure that the pointer is non-null and aligned for T.
/// - The caller must ensure to never use the pointer after the drop.
unsafe fn drop_fn<T>(ptr: NonNull<u8>) {
    let ptr = ptr.as_ptr().cast::<T>();
    unsafe { std::ptr::drop_in_place(ptr) };
}

type DefaultHook = Box<dyn Fn(NonNull<u8>)>;
type CloneHook = Box<dyn Fn(ConstNonNull<u8>, NonNull<u8>)>;
type AddRemoveHook = Box<dyn FnMut(Entity, NonNull<u8>)>;

pub struct TypeHooksBuilder<T> 
{
    default: Option<DefaultHook>,
    clone: Option<CloneHook>,
    on_add: Option<AddRemoveHook>,
    on_remove: Option<AddRemoveHook>,
    phantom: PhantomData<fn(Entity, &mut T)>
}

impl <T: ComponentValue> TypeHooksBuilder<T>
{
    pub fn new() -> Self {
        Self {
            default: None,
            clone: None,
            on_add: None,
            on_remove: None,
            phantom: PhantomData,
        }
    }

    fn with_default(mut self, f: fn() -> T) -> Self {
        self.default = Some(Box::new(move |ptr|{
            let ptr = ptr.as_ptr().cast::<T>();
            unsafe { ptr.write(f());}
        }));

        self
    }

    fn with_clone(mut self, f: fn(&T) -> T) -> Self {
        self.clone = Some(Box::new(move |src, dst|{
            let src = src.as_ptr().cast::<T>();
            let dst = dst.as_ptr().cast::<T>();
            unsafe { dst.write(f(& (*src)));}
        }));

        self
    }

    fn with_add<F>(mut self, mut f: F) -> Self 
    where F: FnMut(Entity, &mut T) + 'static {
        self.on_add = Some(Box::new(move |entity, ptr| {
            let ptr = ptr.as_ptr().cast::<T>();
            f(entity, unsafe { &mut (*ptr) });
        }));
        self
    }

    fn with_remove<F>(mut self, mut f: F) -> Self 
    where F: FnMut(Entity, &mut T) + 'static {
        self.on_add = Some(Box::new(move |entity, ptr| {
            let ptr = ptr.as_ptr().cast::<T>();
            f(entity, unsafe { &mut (*ptr) });
        }));
        self
    }

    fn build(self) -> TypeHooks {
        TypeHooks {
            move_fn: move_fn::<T>,
            drop_fn: drop_fn::<T>,
            default: self.default,
            clone:self.clone,
            on_add: self.on_add,
            on_remove: self.on_remove,
        }
    }
}


pub struct TypeHooks {
    /// Built-in move function for components.
    /// 
    /// # Safety
    /// - The caller must ensure that the pointers are non-null and aligned for data type.
    /// - The caller must ensure to never use `src` after the move.
    pub(crate) move_fn: unsafe fn (src: NonNull<u8>, dst: NonNull<u8>),

    /// Built-in drop function for components.
    /// 
    /// # Safety
    /// - The caller must ensure that the pointer is non-null and aligned for the data type.
    /// - The caller must ensure to never use the pointer after the drop.
    pub(crate) drop_fn: unsafe fn (ptr: NonNull<u8>),

    pub(crate) default: Option<DefaultHook>,
    pub(crate) clone: Option<CloneHook>,
    pub(crate) on_add: Option<AddRemoveHook>,
    pub(crate) on_remove: Option<AddRemoveHook>,
}

impl TypeHooks {
    pub fn new<T>() -> Self {
        Self { 
            move_fn: move_fn::<T>,
            drop_fn: drop_fn::<T>,
            default: None,
            clone: None,
            on_add: None,
            on_remove: None,
        }
    }
}

pub struct TypeInfo {
    id: Id,
    layout: Layout,
    type_name: Option<Box<str>>,
    pub(crate) hooks: TypeHooks,
}

impl TypeInfo {
    #[inline]
    pub fn new<T: ComponentValue>(id: Id, name: Option<impl Into<Box<str>>>) -> Self {
        const_assert!(|T| size_of::<T>() != 0, "can't create type info for ZST");

        Self {
            id,
            layout: Layout::new::<T>(),
            type_name: name.map(Into::into),
            hooks: TypeHooksBuilder::<T>::new().build(),
        }
    }

    pub fn new_untyped(id: Id, layout: Layout, name: Option<impl Into<Box<str>>>, hooks: TypeHooks) -> Self {
        Self {
            id,
            layout,
            type_name: name.map(|n| n.into()),
            hooks,
        }
    }

    #[inline]
    pub fn id(&self) -> Id {
        self.id
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.layout.size()
    }

    #[inline]
    pub fn align(&self) -> usize {
        self.layout.align()
    }

    #[inline]
    pub fn size_align(&self) -> (usize, usize) {
        (self.size(), self.align())
    }
}

/// Sorted list of ids in an [Arcehetype](crate::storage::archetype::Archetype)
#[derive(Hash, PartialEq, Eq, Clone)]
pub struct Type(Rc<[Id]>);

impl Type {
    #[inline]
    pub fn ids(&self) -> &[Id] {
        &self.0
    }

    #[inline]
    pub fn id_count(&self) -> usize {
        self.0.len()
    }
}

impl From<Vec<Id>> for Type {
    fn from(value: Vec<Id>) -> Self {
        Self(value.into())
    }
}

impl Type {
    /// Creates a new sorted type from [Type] and new id.
    ///
    /// Returns [None] if the source type already contains id.
    pub fn extend_with(&self, with: Id) -> Option<Self> {
        /// Find location where to insert id into type
        fn find_type_insert(ids: &[Id], to_add: Id) -> Option<usize> {
            for (i, &id) in ids.iter().enumerate() {
                if id == to_add {
                    return None;
                }
                if id > to_add {
                    return Some(i);
                }
            }

            Some(ids.len())
        }

        let at = find_type_insert(self, with)?;
        let src_array = self.ids();
        let mut dst_array = Vec::with_capacity(src_array.len() + 1);

        dst_array.extend_from_slice(&src_array[..at]);
        dst_array.push(with);
        dst_array.extend_from_slice(&src_array[at..]);

        Some(dst_array.into())
    }
}

impl Deref for Type {
    type Target = [Id];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
pub struct TypeMap {
    pub ids: HashMap<core::any::TypeId, Id>
}

impl TypeMap {
    pub(crate) fn entry<T: ComponentValue>(&mut self) -> Entry<core::any::TypeId, Id> {
       self.ids.entry(core::any::TypeId::of::<T>())
    }

    #[inline]
    pub fn get_id<T: ComponentValue>(&self) -> Option<Id> {
        self.ids.get(&core::any::TypeId::of::<T>()).copied()
    }

    #[inline]
    pub(crate) fn set_id<T: ComponentValue>(&mut self, id: Id) {
        self.ids.insert(core::any::TypeId::of::<T>(), id);
    }
}