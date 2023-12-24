use criterion::{black_box, criterion_group, criterion_main, Criterion};

use fleece_rs::Value;

const PERSON_ENCODED: &[u8] = include_bytes!("../1person.fleece");
const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");

fn decode_people(c: &mut Criterion) {
    c.bench_function("fleece_decode", |b| {
        b.iter(|| {
            Value::from_bytes(black_box(PEOPLE_ENCODED));
        })
    });
}

fn decode_people_unchecked(c: &mut Criterion) {
    c.bench_function("fleece_decode_unchecked", |b| {
        b.iter(|| {
            unsafe { Value::from_bytes_unchecked(black_box(PEOPLE_ENCODED)) };
        })
    });
}

criterion_group!(benches, decode_people, decode_people_unchecked);
criterion_main!(benches);
