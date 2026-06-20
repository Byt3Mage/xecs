use crate::access::Access;
#[cfg(feature = "validate")]
use crate::access::AccessType;
use crate::id::Id;

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
#[error("write conflict on component {0:?}")]
pub struct WriteAccessError(pub Id);

/// Check a single access list is internally conflict-free (no write aliases
/// another access of the same id). Called by `AccessList::push`.
#[cfg(feature = "validate")]
#[inline]
pub fn check_push(existing: &[Access], new: Access) -> Result<(), WriteAccessError> {
    for a in existing {
        if a.id == new.id && (a.ty == AccessType::Write || new.ty == AccessType::Write) {
            return Err(WriteAccessError(a.id));
        }
    }
    Ok(())
}

#[cfg(not(feature = "validate"))]
#[inline(always)]
pub fn check_push(_existing: &[Access], _new: Access) -> Result<(), WriteAccessError> {
    Ok(())
}

#[cfg(feature = "validate")]
#[inline]
pub fn check_combined(lists: &[&[Access]]) -> Result<(), WriteAccessError> {
    fn pair_conflict(a: &[Access], b: &[Access]) -> Option<Id> {
        for x in a {
            for y in b {
                if x.id == y.id && (x.ty == AccessType::Write || y.ty == AccessType::Write) {
                    return Some(x.id);
                }
            }
        }
        None
    }

    for (i, a) in lists.iter().enumerate() {
        for b in &lists[i + 1..] {
            if let Some(id) = pair_conflict(a, b) {
                return Err(WriteAccessError(id));
            }
        }
    }
    Ok(())
}

#[cfg(not(feature = "validate"))]
#[inline(always)]
pub fn check_combined(_lists: &[&[Access]]) -> Result<(), WriteAccessError> {
    Ok(())
}
