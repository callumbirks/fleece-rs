use criterion::{black_box, criterion_group, criterion_main, Criterion};

use fleece::{Encoder, SharedKeys, Value};

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

fn decode_people_sharedkeys(c: &mut Criterion) {
    let value = Value::from_bytes(PEOPLE_ENCODED).unwrap();
    let mut encoder = Encoder::new();
    encoder.set_shared_keys(SharedKeys::new());
    encoder.write_fleece(value).unwrap();
    let scope = encoder.finish_scoped();
    let data = scope.data().unwrap();

    c.bench_function("decode_people_sharedkeys", |b| {
        b.iter(|| {
            let _ = Value::from_bytes(&data);
        });
    });
}

criterion_group!(
    decode_benches,
    decode_people,
    decode_people_unchecked,
    decode_people_sharedkeys
);
criterion_main!(decode_benches);
