macro_rules! impl_bitflags {
    ($type:ty) => {
        impl $type {
            #[inline]
            /// Returns an empty set of flags.
            pub const fn empty() -> Self {
                Self(0)
            }
        
            #[inline]
            pub const fn contains(self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }
        
            #[inline]
            pub const fn insert(&mut self, other: Self) {
                self.0 |= other.0;
            }
        
            #[inline]
            pub const fn remove(&mut self, other: Self) {
                self.0 &= !other.0;
            }

            #[inline]
            pub const fn intersects(self, other: Self) -> bool {
                (self.0 & other.0) != 0
            }
        }

        impl std::default::Default for $type {
            fn default() -> Self {
                Self::empty()
            }
        }
        
        impl std::ops::BitOr for $type {
            type Output = Self;
            fn bitor(self, rhs: Self) -> Self {
                Self(self.0 | rhs.0)
            }
        }
        
        impl std::ops::BitOrAssign for $type {
            #[inline]
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }
        
        impl std::ops::BitAnd for $type {
            type Output = Self;
            fn bitand(self, rhs: Self) -> Self {
                Self(self.0 & rhs.0)
            }
        }
        
        impl std::ops::BitAndAssign for $type {
            #[inline]
            fn bitand_assign(&mut self, rhs: Self) {
                self.0 &= rhs.0;
            }
        }
        
        impl std::ops::BitXor for $type {
            type Output = Self;
            fn bitxor(self, rhs: Self) -> Self {
                Self(self.0 ^ rhs.0)
            }
        }
        
        impl std::ops::BitXorAssign for $type {
            #[inline]
            fn bitxor_assign(&mut self, rhs: Self) {
                self.0 ^= rhs.0;
            }
        }
        
        impl std::ops::Not for $type {
            type Output = Self;
            fn not(self) -> Self {
                Self(!self.0)
            }
        }        
    };
}

/// Represents a set of flags for tables, used to define various properties
/// and behaviors of tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableFlags(u64);

impl TableFlags {
    /// Indicates that the table has built-in components.
    pub const HAS_BUILTINS: Self = Self(1 << 1);

    /// Indicates that the table stores prefabs.
    pub const IS_PREFAB: Self = Self(1 << 2);

    /// Indicates that the table has an `IsA` relationship.
    pub const HAS_IS_A: Self = Self(1 << 3);

    /// Indicates that the table has a `ChildOf` relationship.
    pub const HAS_CHILD_OF: Self = Self(1 << 4);

    /// Indicates that the table has components for `(Identifier, Name)`.
    pub const HAS_NAME: Self = Self(1 << 5);

    /// Indicates that the table has pairs.
    pub const HAS_PAIRS: Self = Self(1 << 6);

    /// Indicates that the table has module data.
    pub const HAS_MODULE: Self = Self(1 << 7);

    /// Indicates that the table has the `EcsDisabled` component.
    pub const IS_DISABLED: Self = Self(1 << 8);

    /// Indicates that the table should never be returned by queries.
    pub const NOT_QUERYABLE: Self = Self(1 << 9);

    /// Indicates that the table has constructors.
    pub const HAS_CTORS: Self = Self(1 << 10);

    /// Indicates that the table has destructors.
    pub const HAS_DTORS: Self = Self(1 << 11);

    /// Indicates that the table supports copy semantics.
    pub const HAS_COPY: Self = Self(1 << 12);

    /// Indicates that the table supports move semantics.
    pub const HAS_MOVE: Self = Self(1 << 13);

    /// Indicates that the table supports toggling.
    pub const HAS_TOGGLE: Self = Self(1 << 14);

    /// Indicates that the table has overrides.
    pub const HAS_OVERRIDES: Self = Self(1 << 15);

    pub const HAS_ON_ADD: Self = Self(1 << 16);
    pub const HAS_ON_REMOVE: Self = Self(1 << 17);
    pub const HAS_ON_SET: Self = Self(1 << 18);
    pub const HAS_ON_TABLE_FILL: Self = Self(1 << 19);
    pub const HAS_ON_TABLE_EMPTY: Self = Self(1 << 20);
    pub const HAS_ON_TABLE_CREATE: Self = Self(1 << 21);
    pub const HAS_ON_TABLE_DELETE: Self = Self(1 << 22);
    pub const HAS_SPARSE: Self = Self(1 << 23);
    pub const HAS_UNION: Self = Self(1 << 24);
}

impl_bitflags!(TableFlags);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityFlags(u64);

impl EntityFlags {
    pub const IS_ID: Self = Self(1 << 0);
    pub const IS_TARGET: Self = Self(1 << 1);
    pub const IS_TRAVERSABLE: Self = Self(1 << 2);
    pub const HAS_SPARSE: Self = Self(1 << 3);
}

impl_bitflags!(EntityFlags);


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentFlags(u64);

impl ComponentFlags {
    // OnDelete behavior flags
    pub const ON_DELETE_REMOVE: Self = Self(1 << 0);
    pub const ON_DELETE_DELETE: Self = Self(1 << 1);
    pub const ON_DELETE_PANIC: Self = Self(1 << 2);
    pub(crate) const ON_DELETE_MASK: Self = Self(
        Self::ON_DELETE_REMOVE.0 | Self::ON_DELETE_DELETE.0 | Self::ON_DELETE_PANIC.0
    );

    // OnDeleteObject behavior flags
    pub const ON_DELETE_OBJECT_REMOVE: Self = Self(1 << 3);
    pub const ON_DELETE_OBJECT_DELETE: Self = Self(1 << 4);
    pub const ON_DELETE_OBJECT_PANIC: Self = Self(1 << 5);
    pub const ON_DELETE_OBJECT_MASK: Self = Self(
        Self::ON_DELETE_OBJECT_REMOVE.0 | Self::ON_DELETE_OBJECT_DELETE.0 | Self::ON_DELETE_OBJECT_PANIC.0
    );

    // OnInstantiate behavior flags
    pub const ON_INSTANTIATE_OVERRIDE: Self = Self(1 << 6);
    pub const ON_INSTANTIATE_INHERIT: Self = Self(1 << 7);
    pub const ON_INSTANTIATE_DONT_INHERIT: Self = Self(1 << 8);
    pub const ON_INSTANTIATE_MASK: Self = Self(
        Self::ON_INSTANTIATE_OVERRIDE.0 | Self::ON_INSTANTIATE_INHERIT.0 | Self::ON_INSTANTIATE_DONT_INHERIT.0
    );

    // Miscellaneous ID flags
    pub const EXCLUSIVE: Self = Self(1 << 9);
    pub const TRAVERSABLE: Self = Self(1 << 10);
    pub const TAG: Self = Self(1 << 11);
    pub const WITH: Self = Self(1 << 12);
    pub const CAN_TOGGLE: Self = Self(1 << 13);
    pub const IS_TRANSITIVE: Self = Self(1 << 14);
    pub const IS_INHERITABLE: Self = Self(1 << 15);
    pub const IS_RELATIONSHIP: Self = Self(1 << 16);

    // Event flags
    pub(crate) const HAS_ON_ADD: Self = Self(1 << 16); // Same values as table flags
    pub(crate) const HAS_ON_REMOVE: Self = Self(1 << 17);
    pub(crate) const HAS_ON_SET: Self = Self(1 << 18);
    pub(crate) const HAS_ON_TABLE_CREATE: Self = Self(1 << 21);
    pub(crate) const HAS_ON_TABLE_DELETE: Self = Self(1 << 22);
    pub(crate) const IS_SPARSE: Self = Self(1 << 23);
    pub(crate) const EVENT_MASK: Self = Self(
        Self::HAS_ON_ADD.0 | Self::HAS_ON_REMOVE.0 | Self::HAS_ON_SET.0 |
        Self::HAS_ON_TABLE_CREATE.0 | Self::HAS_ON_TABLE_DELETE.0 | Self::IS_SPARSE.0
    );

    // Special flag
    pub const MARKED_FOR_DELETE: Self = Self(1 << 30);
}

impl_bitflags!(ComponentFlags);