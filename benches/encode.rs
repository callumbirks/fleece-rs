use criterion::{criterion_group, criterion_main, Criterion};

use fleece_rs::{Encoder, SharedKeys, Value};

const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");

fn encode_people(c: &mut Criterion) {
    let value = Value::from_bytes(PEOPLE_ENCODED).unwrap();
    c.bench_function("encode_people", |b| {
        b.iter(|| {
            let mut encoder = Encoder::new();
            encoder.write_fleece(value).unwrap();
        });
    });
}

fn encode_people_sharedkeys(c: &mut Criterion) {
    let value = Value::from_bytes(PEOPLE_ENCODED).unwrap();
    c.bench_function("encode_people_sharedkeys", |b| {
        b.iter(|| {
            let mut encoder = Encoder::new();
            encoder.set_shared_keys(SharedKeys::new());
            encoder.write_fleece(value).unwrap();
        });
    });
}

criterion_group!(encode_benches, encode_people, encode_people_sharedkeys);
criterion_main!(encode_benches);
