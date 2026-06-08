pub(crate) mod manager;
pub(crate) mod map;

use std::{fmt::Display, ops::Deref, rc::Rc};

/// FFI compatible representation of an id
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(u64);

impl Id {
    /// Built-in entities
    pub const NULL: Id = Id(u64::MAX);

    /// Creates a new `Entity` from raw bits.
    #[inline(always)]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Converts the `Entity` back to raw bits.
    #[inline(always)]
    pub const fn raw(&self) -> u64 {
        self.0
    }

    /// Returns the ID (lower 32 bits).
    #[inline(always)]
    pub const fn idx(&self) -> u32 {
        self.0 as u32
    }

    /// Returns the version (higher 32 bits).
    #[inline(always)]
    pub const fn ver(&self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Increments the version counter.
    pub(crate) const fn next_version(&self) -> Self {
        Self((((self.0 >> 32) + 1) << 32) | (self.idx() as u64))
    }
}

impl From<Id> for usize {
    fn from(value: Id) -> usize {
        value.0 as usize
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}v{}", self.idx(), self.ver())
    }
}

/// Sorted list of ids in a [Table](crate::storage::table::Table)
#[derive(Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct Signature(Rc<[Id]>);

impl Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Clone for Signature {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl From<Vec<Id>> for Signature {
    fn from(mut value: Vec<Id>) -> Self {
        Self({
            value.sort();
            value.dedup();
            value.into()
        })
    }
}

impl<const N: usize> From<[Id; N]> for Signature {
    fn from(value: [Id; N]) -> Self {
        Vec::from(value).into()
    }
}

impl Deref for Signature {
    type Target = [Id];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Signature {
    #[inline]
    pub fn ids(&self) -> &[Id] {
        &self.0
    }

    #[inline]
    pub fn has(&self, id: Id) -> bool {
        self.binary_search(&id).is_ok()
    }

    /// Creates a new sorted list from [Signature] and `with`
    ///
    /// Returns `None` if self already contains `with`.
    pub fn try_extend(&self, with: Id) -> Option<Self> {
        match self.binary_search(&with) {
            Ok(_) => None,
            Err(pos) => Some({
                let mut new_sig = Vec::with_capacity(pos);
                new_sig.extend_from_slice(&self[..pos]);
                new_sig.push(with);
                new_sig.extend_from_slice(&self[pos..]);
                new_sig.into()
            }),
        }
    }

    /// Creates a new sorted list from [Signature] without `from`.
    ///
    /// Returns `None` if self doesn't contain `from`.
    pub fn try_shrink(&self, from: Id) -> Option<Self> {
        match self.binary_search(&from) {
            Ok(pos) => Some({
                let mut new_list = Vec::from(self.as_ref());
                new_list.remove(pos);
                new_list.into()
            }),
            Err(_) => None,
        }
    }
}
