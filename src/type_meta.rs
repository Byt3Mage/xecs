use std::{alloc::Layout, any::TypeId, collections::HashMap, ptr::NonNull};

#[inline]
const fn dtor<T>() -> Option<unsafe fn(ptr: NonNull<u8>)> {
    if std::mem::needs_drop::<T>() {
        Some(|ptr: NonNull<u8>| unsafe { ptr.cast::<T>().drop_in_place() })
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TypeMeta {
    pub dtor: Option<unsafe fn(ptr: NonNull<u8>)>,
    pub dangling: NonNull<u8>,
    pub layout: Layout,
    pub type_id: TypeId,
    pub name: Option<fn() -> &'static str>,
}

impl TypeMeta {
    #[inline]
    pub const fn of<T: 'static>() -> Self {
        Self {
            dtor: dtor::<T>(),
            dangling: NonNull::<T>::dangling().cast(),
            layout: Layout::new::<T>(),
            type_id: TypeId::of::<T>(),
            name: Some(std::any::type_name::<T>),
        }
    }

    #[inline(always)]
    pub fn is<T: 'static>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    #[inline(always)]
    pub fn is_zst(&self) -> bool {
        self.layout.size() == 0
    }

    #[inline(always)]
    pub fn name(&self) -> &'static str {
        self.name.map_or("<anonymous>", |n| n())
    }

    #[inline(always)]
    pub fn assert_type<T: 'static>(&self) {
        assert!(
            self.is::<T>(),
            "type mismatch: expected {}, got {}",
            self.name(),
            std::any::type_name::<T>(),
        )
    }
}

pub struct TypeMap<V>(HashMap<TypeId, V, crate::utils::NoOpHash>);

impl<V> TypeMap<V> {
    #[inline(always)]
    pub fn new() -> Self {
        Self(HashMap::default())
    }

    #[inline(always)]
    pub fn get<T: 'static>(&self) -> Option<&V> {
        self.0.get(&TypeId::of::<T>())
    }

    #[inline(always)]
    pub fn insert<T: 'static>(&mut self, val: V) {
        self.0.insert(TypeId::of::<T>(), val);
    }

    #[inline(always)]
    pub fn remove<T: 'static>(&mut self) {
        self.0.remove(&TypeId::of::<T>());
    }

    #[inline(always)]
    pub fn contains<T: 'static>(&self) -> bool {
        self.0.contains_key(&TypeId::of::<T>())
    }
}
