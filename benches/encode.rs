use criterion::{criterion_group, criterion_main, Criterion};

use fleece_rs::{Encoder, Value};

const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");

fn encode_people(c: &mut Criterion) {
    let value = Value::from_bytes(PEOPLE_ENCODED).unwrap();
    let mut encoder = Encoder::new();
    c.bench_function("encode_people", |b| {
        b.iter(|| {
            encoder.write_fleece(value).unwrap();
            encoder = Encoder::new();
        });
    });
}

criterion_group!(encode_benches, encode_people);
criterion_main!(encode_benches);