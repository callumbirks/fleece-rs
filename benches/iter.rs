use criterion::{black_box, criterion_group, criterion_main, Criterion};

use fleece_rs::Value;

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

criterion_group!(iter_benches, iter_person, iter_people,);
criterion_main!(iter_benches);
