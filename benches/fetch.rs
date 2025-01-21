use criterion::{criterion_group, criterion_main, Criterion};
use fleece::{Encoder, SharedKeys, Value};

const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");
const KEYS: [&str; 10] = [
    "about",
    "age",
    "balance",
    "guid",
    "isActive",
    "latitude",
    "longitude",
    "name",
    "registered",
    "tags",
];

fn fetch_people(c: &mut Criterion) {
    let array = Value::clone_from_bytes(PEOPLE_ENCODED)
        .unwrap()
        .to_array()
        .unwrap();

    c.bench_function("fetch_people", |b| {
        b.iter(|| {
            for value in array.iter() {
                let person = value.as_dict().unwrap();
                for key in KEYS {
                    person.get(key).unwrap();
                }
            }
        });
    });
}

fn fetch_people_sharedkeys(c: &mut Criterion) {
    let scope = {
        let original = Value::from_bytes(PEOPLE_ENCODED).unwrap();
        let mut encoder = Encoder::new();
        encoder.set_shared_keys(SharedKeys::new());
        encoder.write_fleece(original).unwrap();
        encoder.finish_scoped()
    };
    let array = scope.root().unwrap().to_array().unwrap();

    c.bench_function("fetch_people_sharedkeys", |b| {
        b.iter(|| {
            for value in array.iter() {
                let person = value.as_dict().unwrap();
                for key in KEYS {
                    person.get(key).unwrap();
                }
            }
        });
    });
}

fn fetch_people_sharedkeys_hinted(c: &mut Criterion) {
    let scope = {
        let original = Value::from_bytes(PEOPLE_ENCODED).unwrap();
        let mut encoder = Encoder::new();
        encoder.set_shared_keys(SharedKeys::new());
        encoder.write_fleece(original).unwrap();
        encoder.finish_scoped()
    };
    let array = scope.root().unwrap().to_array().unwrap();
    let shared_keys = scope.shared_keys().unwrap();

    c.bench_function("fetch_people_sharedkeys_hinted", |b| {
        b.iter(|| {
            for value in array.iter() {
                let person = value.as_dict().unwrap();
                for key in KEYS {
                    person.get_with_shared_keys(key, shared_keys).unwrap();
                }
            }
        });
    });
}

criterion_group!(
    fetch_benches,
    fetch_people,
    fetch_people_sharedkeys,
    fetch_people_sharedkeys_hinted
);
criterion_main!(fetch_benches);
