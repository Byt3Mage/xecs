use std::{
    alloc::{Layout, LayoutError},
    any::TypeId,
    collections::{HashMap, hash_map::Entry},
    marker::PhantomData,
    mem::needs_drop,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use simple_ternary::tnr;

use crate::{component::Component, id::Entity, type_traits::Data, utils::NoOpHash};

pub type TypeName = String;
type DefaultHook = Box<dyn Fn(NonNull<u8>)>;
type CloneHook = Box<dyn Fn(NonNull<u8>, NonNull<u8>)>;
type SetHook = Box<dyn FnMut(Entity, NonNull<u8>)>;
type RemoveHook = Box<dyn FnMut(Entity, NonNull<u8>)>;

pub struct TypeHooksBuilder<T: Component<DataType = Data>> {
    default: Option<DefaultHook>,
    clone: Option<CloneHook>,
    on_set: Option<SetHook>,
    on_remove: Option<RemoveHook>,
    phantom: PhantomData<fn(&mut T)>,
}

impl<T: Component<DataType = Data>> TypeHooksBuilder<T> {
    pub const fn new() -> Self {
        Self {
            default: None,
            clone: None,
            on_set: None,
            on_remove: None,
            phantom: PhantomData,
        }
    }

    pub fn with_default(mut self, f: fn() -> T) -> Self {
        self.default = Some(Box::new(move |ptr| unsafe {
            ptr.as_ptr().cast::<T>().write(f());
        }));
        self
    }

    pub fn with_clone(mut self, f: fn(&T) -> T) -> Self {
        self.clone = Some(Box::new(move |src, dst| {
            let src = src.cast::<T>();
            let dst = dst.cast::<T>();
            unsafe {
                dst.write(f(src.as_ref()));
            }
        }));

        self
    }

    pub fn on_set(mut self, mut f: impl FnMut(Entity, &mut T) + 'static) -> Self {
        self.on_set = Some(Box::new(move |entity, ptr| {
            f(entity, unsafe { ptr.cast::<T>().as_mut() });
        }));
        self
    }

    pub fn on_remove(mut self, mut f: impl FnMut(Entity, &mut T) + 'static) -> Self {
        self.on_remove = Some(Box::new(move |entity, ptr| {
            f(entity, unsafe { ptr.cast::<T>().as_mut() })
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
    pub(crate) drop_fn: Option<unsafe fn(ptr: NonNull<u8>)>,
    pub(crate) dangling: fn() -> NonNull<u8>,
    pub(crate) arr_layout: fn(n: usize) -> Result<Layout, LayoutError>,
    pub(crate) type_id: TypeId,
    pub(crate) type_name: fn() -> &'static str,
    pub(crate) size: usize,
    pub(crate) align: usize,
    pub(crate) hooks: TypeHooks,
}

impl TypeInfo {
    pub const fn of<T: 'static>(hooks: TypeHooks) -> Self {
        fn drop_fn<U>(ptr: NonNull<u8>) {
            unsafe { ptr.cast::<U>().drop_in_place() };
        }

        Self {
            drop_fn: tnr! {needs_drop::<T>() => Some(drop_fn::<T>) : None},
            dangling: || NonNull::<T>::dangling().cast(),
            arr_layout: Layout::array::<T>,
            type_name: std::any::type_name::<T>,
            type_id: TypeId::of::<T>(),
            size: std::mem::size_of::<T>(),
            align: std::mem::align_of::<T>(),
            hooks,
        }
    }

    #[inline]
    pub fn is<T: 'static>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
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
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            types: HashMap::default(),
        }
    }

    #[inline(always)]
    pub fn get<T: 'static>(&self) -> Option<&V> {
        self.types.get(&TypeId::of::<T>())
    }

    #[inline(always)]
    pub fn insert<T: 'static>(&mut self, val: V) {
        self.types.insert(TypeId::of::<T>(), val);
    }

    #[inline(always)]
    pub fn remove<T: 'static>(&mut self) {
        self.types.remove(&TypeId::of::<T>());
    }

    #[inline(always)]
    pub fn contains<T: 'static>(&self) -> bool {
        self.types.contains_key(&TypeId::of::<T>())
    }

    #[inline(always)]
    pub fn entry<T: 'static>(&mut self) -> Entry<TypeId, V> {
        self.types.entry(TypeId::of::<T>())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct TypeIndex(usize);

impl TypeIndex {
    pub const INVALID: Self = TypeIndex(usize::MAX);

    #[inline(always)]
    pub fn get(self) -> usize {
        self.0
    }
}

pub fn allocate_type_index() -> TypeIndex {
    static MAX_INDEX: AtomicUsize = AtomicUsize::new(0);
    TypeIndex(MAX_INDEX.fetch_add(1, Ordering::Relaxed))
}
