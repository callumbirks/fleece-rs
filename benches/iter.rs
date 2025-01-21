use criterion::{black_box, criterion_group, criterion_main, Criterion};

use fleece::{mutable::MutableArray, Encoder, SharedKeys, Value};

const PERSON_ENCODED: &[u8] = include_bytes!("../1person.fleece");
const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");

fn iter_person(c: &mut Criterion) {
    let dict = Value::from_bytes(black_box(PERSON_ENCODED))
        .unwrap()
        .as_dict()
        .unwrap();
    c.bench_function("iter_person", |b| {
        b.iter(|| {
            assert_eq!(dict.into_iter().count(), 21);
        });
    });
}

fn iter_people(c: &mut Criterion) {
    let array = Value::from_bytes(black_box(PEOPLE_ENCODED))
        .unwrap()
        .as_array()
        .unwrap();
    c.bench_function("iter_people", |b| {
        b.iter(|| {
            assert_eq!(array.into_iter().count(), 1000);
        });
    });
}

fn iter_people_mutable(c: &mut Criterion) {
    let array = Value::clone_from_bytes(black_box(PEOPLE_ENCODED))
        .unwrap()
        .to_array()
        .unwrap();
    let array = MutableArray::from(array);
    c.bench_function("iter_people_mutable", |b| {
        b.iter(|| {
            assert_eq!(array.into_iter().count(), 1000);
        });
    });
}

fn iter_people_shared_keys(c: &mut Criterion) {
    let value = Value::from_bytes(PEOPLE_ENCODED).unwrap();
    let mut encoder = Encoder::new();
    encoder.set_shared_keys(SharedKeys::new());
    encoder.write_fleece(value).unwrap();
    let scope = encoder.finish_scoped();
    let scoped_value = scope.root().unwrap();
    let array = scoped_value.as_array().unwrap();
    c.bench_function("iter_people_sharedkeys", |b| {
        b.iter(|| {
            assert_eq!(array.into_iter().count(), 1000);
        });
    });
}

criterion_group!(
    iter_benches,
    iter_person,
    iter_people,
    iter_people_mutable,
    iter_people_shared_keys
);
criterion_main!(iter_benches);
