use crate::type_meta::TypeMeta;

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

/// Assert a row index does not exceed column bounds.

#[cfg(feature = "validate")]
#[inline]
pub fn check_row_bounds(row: usize, count: usize) {
    assert!(row < count, "xecs: row index out of bounds: row={row} count={count}",);
}

#[cfg(not(feature = "validate"))]
#[inline(always)]
pub fn check_row_bounds(row: usize, count: usize) {}
