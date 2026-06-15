use crate::type_meta::TypeMeta;
use crate::{access::AccessType, component::GetMulti};

/// Assert a column's stored type matches `T`. Called before any typed
/// column access (get/get_mut/write/replace).
#[cfg(feature = "validate")]
#[inline]
pub fn check_type<T: 'static>(meta: &TypeMeta) {
    assert!(
        meta.is::<T>(),
        "component type mismatch: column stores {}, accessed as {}",
        meta.name(),
        std::any::type_name::<T>(),
    );
}

#[cfg(not(feature = "validate"))]
#[inline(always)]
pub fn check_type<T: 'static>(_meta: &TypeMeta) {}

/// Validate a multi-get's accesses are internally aliasing-free: a component
/// accessed mutably must not also be accessed (read or write) in the same get.
#[cfg(feature = "validate")]
#[inline]
pub fn check_multi_get<T: GetMulti>() {
    let accesses = T::accesses();

    for (i, a) in accesses.iter().enumerate() {
        for b in &accesses[i + 1..] {
            if a.id == b.id && (a.ty == AccessType::Write || b.ty == AccessType::Write) {
                panic!("multi-get aliasing: component {:?} borrowed mutably and again", a.id);
            }
        }
    }
}

#[cfg(not(feature = "validate"))]
#[inline(always)]
pub fn check_multi_get<T: GetMulti>() {}
