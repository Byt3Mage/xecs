use crate::{
    component::ComponentValue, entity::Entity, pointer::ConstNonNull, type_impl::TypeImpl,
};
use const_assert::const_assert;
use std::{
    alloc::Layout,
    any::TypeId,
    marker::PhantomData,
    ops::Deref,
    ptr::{self, NonNull},
    rc::Rc,
};

pub type TypeName = String;
type DefaultHook = Box<dyn Fn(NonNull<u8>)>;
type CloneHook = Box<dyn Fn(ConstNonNull<u8>, NonNull<u8>)>;
type SetHook = Box<dyn FnMut(Entity, NonNull<u8>)>;
type RemoveHook = Box<dyn FnMut(Entity, NonNull<u8>)>;

pub struct TypeHooksBuilder<C> {
    default: Option<DefaultHook>,
    clone: Option<CloneHook>,
    on_set: Option<SetHook>,
    on_remove: Option<RemoveHook>,
    phantom: PhantomData<fn(&mut C)>,
}

impl<C: ComponentValue> TypeHooksBuilder<C> {
    pub const fn new() -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't create type hooks for ZST");

        Self {
            default: None,
            clone: None,
            on_set: None,
            on_remove: None,
            phantom: PhantomData,
        }
    }

    pub fn with_default(mut self, f: fn() -> C) -> Self {
        self.default = Some(Box::new(move |ptr| unsafe {
            ptr.as_ptr().cast::<C>().write(f());
        }));
        self
    }

    pub fn with_clone(mut self, f: fn(&C) -> C) -> Self {
        self.clone = Some(Box::new(move |src, dst| {
            let src = src.cast::<C>();
            let dst = dst.cast::<C>();
            unsafe {
                dst.write(f(src.as_ref()));
            }
        }));

        self
    }

    pub fn with_set<F>(mut self, mut f: F) -> Self
    where
        F: FnMut(Entity, &mut C) + 'static,
    {
        self.on_set = Some(Box::new(move |entity, ptr| {
            f(entity, unsafe { ptr.cast().as_mut() })
        }));
        self
    }

    pub fn with_remove<F>(mut self, mut f: F) -> Self
    where
        F: FnMut(Entity, &mut C) + 'static,
    {
        self.on_remove = Some(Box::new(move |entity, ptr| {
            f(entity, unsafe { ptr.cast().as_mut() })
        }));
        self
    }

    pub fn build(self) -> TypeHooks {
        TypeHooks {
            default: self.default,
            clone: self.clone,
            on_set: self.on_set,
            on_remove: self.on_remove,
        }
    }
}

pub struct TypeHooks {
    pub(crate) default: Option<DefaultHook>,
    pub(crate) clone: Option<CloneHook>,
    pub(crate) on_set: Option<SetHook>,
    pub(crate) on_remove: Option<RemoveHook>,
}

pub struct TypeInfo {
    /// Built-in drop function for the type.
    ///
    /// # Safety
    /// - The caller must ensure that the pointer is non-null and aligned for the data type.
    /// - The caller must ensure to never use the pointer after the drop.
    pub(crate) drop_fn: unsafe fn(ptr: *mut u8),
    pub(crate) layout: Layout,
    pub(crate) type_name: &'static str,
    pub(crate) type_id: TypeId,
}

impl TypeInfo {
    pub(crate) fn new<T: 'static>() -> Self {
        Self {
            drop_fn: |ptr| unsafe { ptr::drop_in_place(ptr.cast::<T>()) },
            layout: Layout::new::<T>(),
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
        }
    }

    #[inline]
    pub fn of<T: TypeImpl>() -> &'static TypeInfo {
        T::type_info()
    }

    #[inline]
    pub fn is<T: 'static>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    #[inline]
    pub const fn size(&self) -> usize {
        self.layout.size()
    }

    #[inline]
    pub const fn align(&self) -> usize {
        self.layout.align()
    }

    #[inline]
    pub const fn size_align(&self) -> (usize, usize) {
        (self.size(), self.align())
    }
}

/// Sorted list of ids in an [Arcehetype](crate::storage::table::table)
#[derive(Hash, PartialEq, Eq)]
pub struct Type(Rc<[Entity]>);

impl Clone for Type {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl From<Vec<Entity>> for Type {
    fn from(value: Vec<Entity>) -> Self {
        Self(value.into())
    }
}

impl Deref for Type {
    type Target = [Entity];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Type {
    #[inline]
    pub fn ids(&self) -> &[Entity] {
        &self.0
    }

    #[inline]
    pub fn id_count(&self) -> usize {
        self.0.len()
    }

    /// Creates a new sorted type from [Type] and new id.
    ///
    /// Returns [None] if the source type already contains id.
    pub fn extend_with(&self, with: Entity) -> Option<Self> {
        /// Find location where to insert id into type
        fn find_type_insert(ids: &[Entity], to_add: Entity) -> Option<usize> {
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
