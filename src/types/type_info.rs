use crate::{component::ComponentValue, entity::Entity, pointer::ConstNonNull};
use const_assert::const_assert;
use std::{
    alloc::Layout,
    any::TypeId,
    marker::PhantomData,
    ptr::{self, NonNull},
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
        const_assert!(|C| size_of::<C>() != 0, "can't create default for ZST");
        self.default = Some(Box::new(move |ptr| unsafe {
            ptr.as_ptr().cast::<C>().write(f());
        }));
        self
    }

    pub fn with_clone(mut self, f: fn(&C) -> C) -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't create clone for ZST");
        self.clone = Some(Box::new(move |src, dst| {
            let src = src.cast::<C>();
            let dst = dst.cast::<C>();
            unsafe {
                dst.write(f(src.as_ref()));
            }
        }));

        self
    }

    pub fn on_set(mut self, mut f: impl FnMut(Entity, &mut C) + 'static) -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't create set hook for ZST");
        self.on_set = Some(Box::new(move |entity, ptr| {
            f(entity, unsafe { ptr.cast().as_mut() })
        }));
        self
    }

    pub fn on_remove(mut self, mut f: impl FnMut(Entity, &mut C) + 'static) -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't create remove hook for ZST");
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

impl TypeHooks {
    #[inline]
    pub(crate) fn empty() -> Self {
        Self {
            default: None,
            clone: None,
            on_set: None,
            on_remove: None,
        }
    }
}

pub struct TypeInfo {
    pub(crate) drop_fn: Option<unsafe fn(ptr: NonNull<u8>)>,
    pub(crate) dangling: fn() -> NonNull<u8>,
    pub(crate) layout: Layout,
    pub(crate) type_name: &'static str,
    pub(crate) type_id: TypeId,
    pub(crate) hooks: TypeHooks,
}

impl TypeInfo {
    pub(crate) fn new<C: ComponentValue>(hooks: TypeHooksBuilder<C>) -> Self {
        fn drop_impl<T>(ptr: NonNull<u8>) {
            let ptr = ptr.as_ptr().cast::<T>();
            unsafe { ptr::drop_in_place(ptr) };
        }

        Self {
            drop_fn: const {
                if std::mem::needs_drop::<C>() {
                    Some(drop_impl::<C>)
                } else {
                    None
                }
            },
            dangling: || NonNull::<C>::dangling().cast(),
            layout: Layout::new::<C>(),
            type_name: std::any::type_name::<C>(),
            type_id: TypeId::of::<C>(),
            hooks: hooks.build(),
        }
    }

    #[inline]
    pub fn is<C: ComponentValue>(&self) -> bool {
        self.type_id == TypeId::of::<C>()
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
