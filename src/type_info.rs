use const_assert::const_assert;
use std::{alloc::Layout, any::TypeId, collections::HashMap, marker::PhantomData, ops::Deref, ptr::NonNull, rc::Rc};
use crate::{component::ComponentValue, entity::Entity, id::Id, pointer::ConstNonNull};

pub type TypeName = Box<str>;

/// Built-in move function for components.
/// 
/// # Safety
/// - The caller must ensure that the pointers are non-null and aligned for T.
/// - The caller must ensure to never use src after the move.
unsafe fn move_fn<T: ComponentValue>(src: NonNull<u8>, dst: NonNull<u8>) {
    let src = src.cast::<T>();
    let dst = dst.cast::<T>();
    unsafe { dst.write(src.read()) };
}

/// Built-in drop function for components.
/// 
/// # Safety
/// - The caller must ensure that the pointer is non-null and aligned for T.
/// - The caller must ensure to never use the pointer after the drop.
unsafe fn drop_fn<T>(ptr: NonNull<u8>) {
    unsafe { ptr.cast::<T>().drop_in_place(); };
}

type DefaultHook = Box<dyn Fn(NonNull<u8>)>;
type CloneHook = Box<dyn Fn(ConstNonNull<u8>, NonNull<u8>)>;
type AddHook = Box<dyn FnMut(Entity)>;
type SetHook = Box<dyn FnMut(Entity, NonNull<u8>)>;
type RemoveHook = Box<dyn FnMut(Entity, NonNull<u8>)>;

pub struct TypeHooksBuilder<C> 
{
    default: Option<DefaultHook>,
    clone: Option<CloneHook>,
    on_add: Option<AddHook>,
    on_set: Option<SetHook>,
    on_remove: Option<RemoveHook>,
    phantom: PhantomData<fn(Entity, &mut C)>
}

impl <C: ComponentValue> TypeHooksBuilder<C>
{
    pub const fn new() -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't create type hooks for ZST");

        Self {
            default: None,
            clone: None,
            on_add: None,
            on_set: None,
            on_remove: None,
            phantom: PhantomData,
        }
    }

    pub fn with_default(mut self, f: fn() -> C) -> Self {
        self.default = Some(Box::new(move |ptr|{
            let ptr = ptr.as_ptr().cast::<C>();
            unsafe { ptr.write(f());}
        }));

        self
    }

    pub fn with_clone(mut self, f: fn(&C) -> C) -> Self {
        self.clone = Some(Box::new(move |src, dst|{
            let src = src.cast::<C>();
            let dst = dst.cast::<C>();
            unsafe { dst.write(f(src.as_ref()));}
        }));

        self
    }

    pub fn with_add<F>(mut self, mut f: F) -> Self 
    where F: FnMut(Entity) + 'static {
        self.on_add = Some(Box::new(move |entity| f(entity)));
        self
    }

    pub fn with_set<F>(mut self, mut f: F) -> Self 
    where F: FnMut(Entity, &mut C) + 'static {
        self.on_set = Some(Box::new(move |entity, ptr| f(entity, unsafe { ptr.cast().as_mut() })));
        self
    }

    pub fn with_remove<F>(mut self, mut f: F) -> Self 
    where F: FnMut(Entity, &mut C) + 'static {
        self.on_remove = Some(Box::new(move |entity, ptr| f(entity, unsafe { ptr.cast().as_mut() })));
        self
    }

    pub fn build(self) -> TypeHooks {
        TypeHooks {
            move_fn: move_fn::<C>,
            drop_fn: drop_fn::<C>,
            default: self.default,
            clone:self.clone,
            on_add: self.on_add,
            on_set: self.on_set,
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
    pub(crate) on_add: Option<AddHook>,
    pub(crate) on_set: Option<SetHook>,
    pub(crate) on_remove: Option<RemoveHook>,
}

impl TypeHooks {
    pub fn new<C: ComponentValue>() -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't create type hooks for ZST");

        Self {
            move_fn: move_fn::<C>,
            drop_fn: drop_fn::<C>,
            default: None,
            clone:None,
            on_add: None,
            on_set: None,
            on_remove: None,
        }
    }
}

pub struct TypeInfo {
    pub(crate) id: Id,
    pub(crate) layout: Layout,
    pub(crate) hooks: TypeHooks,
    pub(crate) type_name: Option<TypeName>,
    pub(crate) type_id: TypeId,
}

impl TypeInfo {
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
    pub(crate) fn new() -> Self {
        Self {
            ids: HashMap::new()
        }
    }

    #[inline]
    pub fn get(&self, ty_id: core::any::TypeId) -> Option<Id> {
        self.ids.get(&ty_id).copied()
    }

    #[inline]
    pub fn get_t<T: ComponentValue>(&self) -> Option<Id> {
        self.get(core::any::TypeId::of::<T>())
    }

    #[inline]
    pub(crate) fn set_t<T: ComponentValue>(&mut self, id: Id) {
        self.set(TypeId::of::<T>(), id);
    }

    #[inline]
    pub(crate) fn set(&mut self, ty_id: TypeId, id: Id) {
        self.ids.insert(ty_id, id);
    }

    #[inline]
    pub fn has(&self, ty_id: TypeId) -> bool {
        self.ids.contains_key(&ty_id)
    }

    #[inline]
    pub fn has_t<T: ComponentValue>(&self) -> bool {
        self.has(TypeId::of::<T>())
    }
}