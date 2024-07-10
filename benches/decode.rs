use criterion::{black_box, criterion_group, criterion_main, Criterion};

use fleece_rs::Value;

const PERSON_ENCODED: &[u8] = include_bytes!("../1person.fleece");
const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");

fn decode_people(c: &mut Criterion) {
    c.bench_function("decode_people", |b| {
        b.iter(|| {
            let _ = Value::from_bytes(black_box(PEOPLE_ENCODED));
        });
    });
}

fn decode_people_unchecked(c: &mut Criterion) {
    c.bench_function("decode_people_unchecked", |b| {
        b.iter(|| {
            let _ = unsafe { Value::from_bytes_unchecked(black_box(PEOPLE_ENCODED)) };
        });
    });
}

criterion_group!(decode_benches, decode_people, decode_people_unchecked,);
criterion_main!(decode_benches);
