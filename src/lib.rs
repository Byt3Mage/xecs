pub mod macros;
pub mod world;

mod component;
mod component_index;
mod data_structures;
mod dynamic_struct;
mod entity;
mod entity_index;
mod entity_view;
mod error;
mod flags;
mod graph;
mod id;
mod pointer;
mod storage;
mod type_info;
mod world_utils;
mod memory;

#[cfg(test)]
mod tests;

#[inline]
pub fn fibonacci(n: u64) -> u64 {
    let mut a = 0;
    let mut b = 1;

    match n {
        0 => b,
        _ => {
            for _ in 0..n {
                let c = a + b;
                a = b;
                b = c;
            }
            b
        }
    }
}