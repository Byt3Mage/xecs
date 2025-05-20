use crate::{component::Component, id::Id, pointer::ConstNonNull};
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
type SetHook = Box<dyn FnMut(Id, NonNull<u8>)>;
type RemoveHook = Box<dyn FnMut(Id, NonNull<u8>)>;

pub struct TypeHooksBuilder<C> {
    default: Option<DefaultHook>,
    clone: Option<CloneHook>,
    on_set: Option<SetHook>,
    on_remove: Option<RemoveHook>,
    phantom: PhantomData<fn(&mut C)>,
}

impl<C: Component> TypeHooksBuilder<C> {
    pub const fn new() -> Self {
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

    pub fn on_set(mut self, mut f: impl FnMut(Id, &mut C) + 'static) -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't create set hook for ZST");
        self.on_set = Some(Box::new(move |entity, ptr| {
            f(entity, unsafe { ptr.cast().as_mut() })
        }));
        self
    }

    pub fn on_remove(mut self, mut f: impl FnMut(Id, &mut C) + 'static) -> Self {
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

pub struct TypeInfo {
    pub(crate) drop_fn: Option<unsafe fn(ptr: *mut u8)>,
    pub(crate) layout: Layout,
    pub(crate) type_id: TypeId,
    pub(crate) type_name: fn() -> &'static str,
    pub(crate) hooks: TypeHooks,
}

impl TypeInfo {
    pub fn of<T: 'static>(hooks: TypeHooksBuilder<T>) -> Option<Self> {
        if size_of::<T>() == 0 {
            return None;
        }

        fn drop_impl<T>(ptr: *mut u8) {
            unsafe { ptr::drop_in_place(ptr.cast::<T>()) };
        }

        Some(Self {
            drop_fn: const {
                if std::mem::needs_drop::<T>() {
                    Some(drop_impl::<T>)
                } else {
                    None
                }
            },
            layout: Layout::new::<T>(),
            type_name: std::any::type_name::<T>,
            type_id: TypeId::of::<T>(),
            hooks: hooks.build(),
        })
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

    #[inline]
    pub fn name(&self) -> &'static str {
        (self.type_name)()
    }
}
