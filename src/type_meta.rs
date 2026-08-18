use std::{alloc::Layout, any::TypeId, mem};

use crate::memory::DropFn;

#[inline]
const fn dtor<T>() -> DropFn {
    if mem::needs_drop::<T>() { Some(|p| unsafe { p.cast::<T>().drop_in_place() }) } else { None }
}

#[derive(Debug, Clone, Copy)]
pub struct TypeMeta {
    pub drop: DropFn,
    pub layout: Layout,
    pub type_id: Option<fn() -> TypeId>,
    pub type_name: Option<fn() -> &'static str>,
}

impl TypeMeta {
    #[inline]
    pub const fn of<T: 'static>() -> Self {
        Self {
            drop: dtor::<T>(),
            layout: Layout::new::<T>(),
            type_id: Some(TypeId::of::<T>),
            type_name: Some(std::any::type_name::<T>),
        }
    }

    #[inline(always)]
    pub const fn is_zst(&self) -> bool {
        self.layout.size() == 0
    }

    #[inline(always)]
    pub fn type_id(&self) -> Option<TypeId> {
        self.type_id.map(|id| id())
    }

    #[inline(always)]
    pub fn type_name(&self) -> Option<&'static str> {
        self.type_name.map(|n| n())
    }
}

pub trait HasMeta: 'static {
    const META: &'static TypeMeta;
}

impl<T: 'static> HasMeta for T {
    const META: &'static TypeMeta = &TypeMeta::of::<Self>();
}
