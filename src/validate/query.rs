use crate::access::{Access, AccessType};

/// Validate a field type's required accesses against
/// the query's declared accesses for a table.
#[cfg(feature = "validate")]
#[inline]
pub fn check_access(required: AccessType, declared: &Access) {
    if required == AccessType::Write && declared.ty == AccessType::Read {
        panic!("access requires write to read-only column {}", declared.id);
    }
}

#[cfg(not(feature = "validate"))]
#[inline(always)]
pub fn check_access(_required: AccessType, _declared: &Access) {}

/// Validate a row type's required accesses against the query's declared
/// accesses for a table. Called once per table in `each_row`, before the loop.
#[cfg(feature = "validate")]
#[inline]
pub fn check_row(required: &[AccessType], declared: &[Access]) {
    let req_len = required.len();
    let dec_len = declared.len();

    assert_eq!(
        req_len, dec_len,
        "row field count ({req_len}) != query column count ({dec_len})",
    );

    for (&req, decl) in required.iter().zip(declared) {
        check_access(req, decl);
    }
}

#[cfg(not(feature = "validate"))]
#[inline(always)]
pub fn check_row(_required: &[AccessType], _declared: &[Access]) {}
