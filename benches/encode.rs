use criterion::{criterion_group, criterion_main, Criterion};

use fleece::{Encoder, Value};

const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");

fn encode_people(c: &mut Criterion) {
    let value = Value::from_bytes(PEOPLE_ENCODED).unwrap();
    c.bench_function("encode_people", |b| {
        b.iter(|| {
            let mut encoder = Encoder::new();
            encoder.write_fleece(value).unwrap();
            let _ = encoder.finish();
        });
    });
}

criterion_group!(encode_benches, encode_people);
criterion_main!(encode_benches);
