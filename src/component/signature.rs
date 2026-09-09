use std::rc::Rc;

use crate::component::id::ComponentId;

/// Sorted and deduped list of component ids in a [Table]
///
/// [Table]: crate::table::Table
#[derive(Debug, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct Signature(Rc<[ComponentId]>);

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Clone for Signature {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T: Into<Vec<ComponentId>>> From<T> for Signature {
    fn from(value: T) -> Self {
        let mut vec = value.into();
        vec.sort_unstable();
        vec.dedup();
        Self(vec.into())
    }
}

impl std::ops::Deref for Signature {
    type Target = [ComponentId];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Signature {
    #[inline]
    pub fn find_id(&self, id: &ComponentId) -> Option<usize> {
        self.binary_search(id).ok()
    }

    #[inline]
    pub fn has_id(&self, id: &ComponentId) -> bool {
        self.binary_search(id).is_ok()
    }

    /// Creates a new sorted list from [Signature] and `with`
    ///
    /// Returns `None` if self already contains `with`.
    pub fn try_extend(&self, with: ComponentId) -> Option<Self> {
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
    pub fn try_shrink(&self, from: ComponentId) -> Option<Self> {
        match self.binary_search(&from) {
            Ok(pos) => Some({
                let mut new_list = Vec::from(self.as_ref());
                new_list.remove(pos);
                new_list.into()
            }),
            Err(_) => None,
        }
    }

    /// Sorted union with `other`, or `None` when `other` is contained in `self`
    /// and the union would therefore equal `self`.
    pub fn try_union(&self, other: &Signature) -> Option<Self> {
        let (mut a, mut b) = (0, 0);

        // Advance while every id in `other` is contained in self.
        // Until an uncontained id appears, the union is `self`.
        while b < other.len() {
            while a < self.len() && self[a] < other[b] {
                a += 1;
            }
            if a == self.len() || self[a] != other[b] {
                break;
            }
            a += 1;
            b += 1;
        }

        if b == other.len() {
            return None;
        }

        // `other[b]` is the first unheld id. Every element of `self[..a]` is
        // below it, and every element of `other[..b]` was matched within that
        // range, so the prefix is already the head of the union.
        //
        // Capacity is an upper bound: `other[b + 1..]` may contain held ids,
        // which dedup to one element each.
        let mut out = Vec::with_capacity(self.len() + (other.len() - b));
        out.extend_from_slice(&self[..a]);

        // Use a branchless merge. On a tie both cursors advance and one copy
        // is pushed, which performs the dedup.
        while a < self.len() && b < other.len() {
            let (x, y) = (self[a], other[b]);
            out.push(x.min(y));
            a += (x <= y) as usize;
            b += (y <= x) as usize;
        }

        // The loop exits when one input is exhausted; at most one tail runs.
        out.extend_from_slice(&self[a..]);
        out.extend_from_slice(&other[b..]);

        Some(Self(out.into())) // ids already sorted
    }

    pub fn try_difference(&self, other: &Signature) -> Option<Self> {
        let (mut a, mut b) = (0, 0);

        // Advance while no id in `other` is contained.
        // Until a contained id appears, the difference is `self`.
        while b < other.len() {
            while a < self.len() && self[a] < other[b] {
                a += 1;
            }
            if a == self.len() || self[a] == other[b] {
                break;
            }
            b += 1;
        }

        // `other` ran out with nothing matched, or `self` ran out
        // below everything left in `other`.
        if a == self.len() || b == other.len() {
            return None;
        }

        // `self[a] == other[b]` is the first id to drop. `self[..a]` is
        // entirely below it and survives. Capacity is an upper bound; the
        // result is a subsequence of `self`.
        let mut out = Vec::with_capacity(self.len() - 1);
        out.extend_from_slice(&self[..a]);
        a += 1;
        b += 1;

        while a < self.len() && b < other.len() {
            // Skip ids below the current one.
            // They're not contained, so they remove nothing.
            while b < other.len() && other[b] < self[a] {
                b += 1;
            }
            if b < other.len() && other[b] == self[a] {
                b += 1;
            } else {
                out.push(self[a]);
            }
            a += 1;
        }

        // `other` is exhausted; everything remaining in `self` survives.
        out.extend_from_slice(&self[a..]);

        Some(Self(out.into())) // ids already sorted
    }
}
