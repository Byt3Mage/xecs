use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use xecs::world::World;
use std::collections::HashMap;

struct Component { id: usize }

pub fn criterion_benchmark(c: &mut Criterion) {    
    let arr = [Component{id: 42}, Component{id: 69}];

    c.bench_with_input(BenchmarkId::new("array_access", "test"), &arr, |b, arr| {
        b.iter(|| {
            // Force the compiler to treat all operations as "unpredictable"
            let idx = black_box(2);
            if black_box(idx < black_box(arr).len()) {
                let component = unsafe { black_box(arr).get_unchecked(black_box(idx)) };
                black_box(component.id == black_box(69))
            }
            else {
                black_box(false)
            }
            
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);