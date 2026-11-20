use crate::{Fixed, Many, One, relation::sealed::Sealed};

pub trait Unbounded: Sealed + 'static {}

impl Unbounded for One {}
impl Unbounded for Many {}

pub trait Bounded: Sealed + 'static {}
impl<const N: usize> Bounded for Fixed<N> {}
