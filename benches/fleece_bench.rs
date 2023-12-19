use criterion::{black_box, criterion_group, criterion_main, Criterion};

use fleece::Value;

const PERSON_ENCODED: &[u8] = include_bytes!("../1person.fleece");
const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");

fn fleece_decode(c: &mut Criterion) {
    c.bench_function("fleece_decode", |b| {
        b.iter(|| {
            assert!(Value::from_data(black_box(PEOPLE_ENCODED)).is_some());
        })
    });
}

fn fleece_decode_unchecked(c: &mut Criterion) {
    c.bench_function("fleece_decode_unchecked", |b| {
        b.iter(|| {
            unsafe { assert!(Value::from_data_unchecked(black_box(PEOPLE_ENCODED)).is_some()) };
        })
    });
}

criterion_group!(benches, fleece_decode, fleece_decode_unchecked);
criterion_main!(benches);
