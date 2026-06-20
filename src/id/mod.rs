pub(crate) mod allocator;
pub(crate) mod map;

use std::{fmt::Display, ops::Deref, rc::Rc};

/// FFI compatible representation of an id
#[derive(Debug, Copy, Clone)]
#[repr(C, align(8))]
pub struct Id {
    #[cfg(target_endian = "little")]
    pub index: u32,
    pub generation: u32,
    #[cfg(target_endian = "big")]
    pub index: u32,
}

impl PartialEq for Id {
    #[inline]
    fn eq(&self, other: &Id) -> bool {
        // By using `to_bits`, the codegen can be optimized out even
        // further potentially. Relies on the correct alignment/field
        // order of `Id`.
        self.to_bits() == other.to_bits()
    }
}

impl Eq for Id {}

impl PartialOrd for Id {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Id {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.to_bits().cmp(&other.to_bits())
    }
}

impl std::hash::Hash for Id {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.to_bits().hash(state);
    }
}

impl Id {
    #[inline(always)]
    pub const fn new(index: u32) -> Self {
        Self { index, generation: 0 }
    }

    /// Creates a new `Id` from raw bits.
    #[inline(always)]
    pub const fn from_bits(bits: u64) -> Self {
        Self {
            index: bits as u32,
            generation: (bits >> 32) as u32,
        }
    }

    /// Converts the `Id` back to raw bits.
    #[inline(always)]
    pub const fn to_bits(self) -> u64 {
        self.index as u64 | ((self.generation as u64) << 32)
    }

    /// Increments the version counter.
    pub(crate) const fn next_generation(self) -> Self {
        Self { generation: self.generation + 1, ..self }
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}v{}", self.index, self.generation)
    }
}

/// Sorted list of component ids in a [Table](crate::storage::table::Table)
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
