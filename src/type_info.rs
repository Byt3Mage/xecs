use crate::{component::Component, id::Id, utils::NoOpHash};
use const_assert::const_assert;
use std::{
    alloc::Layout,
    any::TypeId,
    collections::HashMap,
    marker::PhantomData,
    ptr::{self, NonNull},
};

pub type TypeName = String;
type DefaultHook = Box<dyn Fn(NonNull<u8>)>;
type CloneHook = Box<dyn Fn(NonNull<u8>, NonNull<u8>)>;
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
            f(entity, unsafe { ptr.cast::<C>().as_mut() });
        }));
        self
    }

    pub fn on_remove(mut self, mut f: impl FnMut(Id, &mut C) + 'static) -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't create remove hook for ZST");
        self.on_remove = Some(Box::new(move |entity, ptr| {
            f(entity, unsafe { ptr.cast::<C>().as_mut() })
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
    pub(crate) type_id: fn() -> TypeId,
    pub(crate) type_name: fn() -> &'static str,
    pub(crate) hooks: TypeHooks,
}

impl TypeInfo {
    pub fn of<C: Component>(hooks: TypeHooksBuilder<C>) -> Option<Self> {
        if size_of::<C>() == 0 {
            return None;
        }

        fn drop_impl<T>(ptr: *mut u8) {
            unsafe { ptr::drop_in_place(ptr.cast::<T>()) };
        }

        Some(Self {
            drop_fn: const {
                if std::mem::needs_drop::<C>() {
                    Some(drop_impl::<C>)
                } else {
                    None
                }
            },
            layout: Layout::new::<C>(),
            type_name: std::any::type_name::<C>,
            type_id: TypeId::of::<C>,
            hooks: hooks.build(),
        })
    }

    #[inline]
    pub fn is<C: Component>(&self) -> bool {
        (self.type_id)() == TypeId::of::<C>()
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

pub struct TypeMap<V> {
    types: HashMap<TypeId, V, NoOpHash>,
}

impl<V> TypeMap<V> {
    pub fn new() -> Self {
        Self {
            types: HashMap::default(),
        }
    }

    #[inline]
    pub fn get<T: 'static>(&self) -> Option<&V> {
        self.types.get(&TypeId::of::<T>())
    }

    #[inline]
    pub fn insert<T: 'static>(&mut self, val: V) {
        self.types.insert(TypeId::of::<T>(), val);
    }

    pub fn remove<T: 'static>(&mut self) {
        self.types.remove(&TypeId::of::<T>());
    }

    #[inline]
    pub fn contains<T: 'static>(&self) -> bool {
        self.types.contains_key(&TypeId::of::<T>())
    }
}
