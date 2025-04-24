use crate::entity::{ECS_ANY, ECS_WILDCARD, Entity};

pub type Id = u64;

pub(crate) const HI_COMPONENT_ID: u64 = 256;
pub(crate) const ID_FLAGS_MASK: u64 = 0xFF << 60;
pub(crate) const ENTITY_MASK: u64 = 0xFFFFFFFF;
pub(crate) const GENERATION_MASK: u64 = 0xFFFF << 32;
pub(crate) const COMPONENT_MASK: u64 = !ID_FLAGS_MASK;
pub(crate) const PAIR: Id = 1 << 63;

/// Id flags
pub(crate) const ID_TAG: u64 = 1 << 11;

/* Id flags */
pub(crate) const ECS_PAIR: Id = 1 << 63;
pub(crate) const ECS_AUTO_OVERRIDE: Id = 1 << 62;
pub(crate) const ECS_TOGGLE: Id = 1 << 61;

#[inline]
pub const fn entity_hi(id: Id) -> u32 {
    id as u32
}

#[inline]
pub const fn entity_lo(id: Id) -> u32 {
    (id >> 32) as u32
}

#[inline]
pub const fn entity_comb(lo: Id, hi: Id) -> Id {
    ((hi as u64) << 32) | ((lo as u32) as u64)
}

#[inline]
pub const fn pair(pred: Id, obj: Id) -> Id {
    PAIR | entity_comb(obj, pred)
}

#[inline]
pub const fn is_pair(id: Id) -> bool {
    (id & ID_FLAGS_MASK) == PAIR
}

#[inline]
pub const fn pair_first(id: Id) -> u32 {
    entity_hi(id & COMPONENT_MASK)
}

#[inline]
pub const fn pair_second(id: Id) -> u32 {
    entity_lo(id)
}

#[inline]
pub const fn generation(e: Entity) -> u64 {
    (e & GENERATION_MASK) >> 32
}

pub const fn strip_generation(id: Id) -> Id {
    // If this is not a pair, erase the generation bits
    if (id & ID_FLAGS_MASK) != 0 {
        id & !GENERATION_MASK
    } else {
        id
    }
}

pub const fn is_wildcard(id: Id) -> bool {
    if id == ECS_WILDCARD || id == ECS_ANY {
        return true;
    }

    if !is_pair(id) {
        return false;
    }

    let first = pair_first(id) as u64;
    let second = pair_second(id) as u64;

    first == ECS_WILDCARD || second == ECS_WILDCARD || first == ECS_ANY || second == ECS_ANY
}

#[inline]
pub const fn has_id_flag(e: Id, flag: Id) -> bool {
    (e & flag) != 0
}

#[inline]
pub const fn has_relation(e: Id, rel: Id) -> bool {
    has_id_flag(e, PAIR) && pair_first(e) == (rel as u32)
}
