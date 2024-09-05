use std::collections::BTreeSet;
use std::fs::OpenOptions;

use crate::encoder::Encoder;
use crate::value::{varint, ValueType};

use super::*;

const PERSON_ENCODED: &[u8] = include_bytes!("../1person.fleece");
const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");

fn decode_person_checks(person: &Value) {
    assert_eq!(
        person.value_type(),
        ValueType::Dict,
        "Expected Person to be a Dict!"
    );
    let person_dict = person.as_dict().unwrap();
    assert_eq!(person_dict.len(), 21, "Expected Person to have 21 keys!");
    let age = person_dict
        .get("age")
        .expect("Expected Person to have key 'age'!");
    assert_eq!(
        age.value_type(),
        ValueType::Short,
        "Expected age to be a Short!"
    );
    assert_eq!(age.to_short(), 30, "Expected age to be 30!");
    // Check the iterator also works, and returns the correct number of elements
    assert_eq!(person_dict.into_iter().count(), 21);
}

#[test]
fn decode_person() {
    let person = Value::from_bytes(PERSON_ENCODED);
    let person = person.expect("Failed to decode Fleece");
    decode_person_checks(person);
}

fn decode_people_checks(people: &Value) {
    let people_array = people.as_array().expect("Expected People to be an Array!");
    assert_eq!(people_array.len(), 1000, "Expected 1000 people!");
    for (i, person) in people_array.into_iter().enumerate() {
        let person = person.as_dict().expect("Expected Person to be a Dict!");
        let id = person
            .get("_id")
            .expect("Expected Person to have key '_id'!");
        assert_eq!(
            id.value_type(),
            ValueType::String,
            "Expected _id to be a String!"
        );
        let index = person
            .get("index")
            .expect("Expected Person to have key 'index'!");
        assert_eq!(
            index.value_type(),
            ValueType::Short,
            "Expected index to be a Short!"
        );
        #[allow(clippy::cast_possible_truncation)]
        let index = index.to_unsigned_int() as usize;
        assert_eq!(
            index, i,
            "Expected index to be the same as the array index!"
        );
    }
}

fn varint_test(val: u64) {
    let size_required = varint::size_required(val);
    let mut buf: Vec<u8> = vec![0; size_required];
    let _written = varint::write(&mut buf, val);
    println!("Wrote varint {:02x?}", &buf);
    let (_read, out_val) = varint::read(&buf);
    assert_eq!(val, out_val);
}

#[test]
fn varint() {
    varint_test(8_704_268);
    varint_test(100_000);
    varint_test(603);
    varint_test(87);
}

#[test]
fn decode_people() {
    let people = Value::from_bytes(PEOPLE_ENCODED);
    let people = people.expect("Failed to decode Fleece");

    decode_people_checks(people);
}

#[test]
fn encode_person() {
    let original = Value::from_bytes(PERSON_ENCODED).expect("Failed to decode Fleece");
    let mut encoder = Encoder::new();
    encoder
        .write_fleece(original)
        .expect("Failed to write value!");
    let res = encoder.finish();
    let value = Value::from_bytes(&res).unwrap();

    assert_eq!(res.as_slice(), PERSON_ENCODED);

    decode_person_checks(value);
}

#[test]
fn encode_people() {
    let original = Value::from_bytes(PEOPLE_ENCODED).expect("Failed to decode Fleece");
    let mut encoder = Encoder::new();
    encoder
        .write_fleece(original)
        .expect("Failed to write value!");
    let res = encoder.finish();
    let value = Value::from_bytes(&res).unwrap();
    decode_people_checks(value);
}

#[test]
fn write_to_file() {
    let original = Value::from_bytes(PEOPLE_ENCODED).expect("Failed to decode Fleece");
    let file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("test_1000people.fleece")
        .expect("Failed to open file");
    let mut encoder = Encoder::new_to_writer(file);
    encoder
        .write_fleece(original)
        .expect("Failed to write Fleece!");
    encoder.finish();

    let file_bytes = std::fs::read("test_1000people.fleece").expect("Failed to read file");
    let result = Value::from_bytes(&file_bytes).expect("Failed to decode value");
    decode_people_checks(result);
    std::fs::remove_file("test_1000people.fleece").ok();
}

#[test]
fn encoder_multiple_top_level_collections() {
    let mut encoder = Encoder::new();
    encoder.begin_array(1).unwrap();
    encoder.write_value(&42).unwrap();
    encoder.end_array().unwrap();
    assert!(matches!(
        encoder.begin_array(1),
        Err(encoder::EncodeError::MultiTopLevelCollection)
    ));
}

fn write_10_000() {
    let original = Value::from_bytes(PEOPLE_ENCODED).unwrap();
    let file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("10_000people.fleece")
        .unwrap();
    let mut encoder = Encoder::new_to_writer(file);
    let array = original.as_array().unwrap();
    encoder.begin_array(10_000).unwrap();
    for _ in 0..10 {
        for value in array {
            encoder.write_fleece(value).unwrap();
        }
    }
    encoder.end_array().unwrap();
    encoder.finish();
}

// A larger read/write test which should catch any bugs related to wide arrays / pointers.
#[test]
fn decode_10_000() {
    write_10_000();
    let bytes = std::fs::read("10_000people.fleece").expect("Failed to read file");
    let value = Value::from_bytes(&bytes).expect("Failed to parse Value from bytes");
    assert_eq!(value.value_type(), ValueType::Array);
    let array = value.as_array().unwrap();
    assert_eq!(array.len(), 1000 * 10);

    for person in array {
        let person = person.as_dict().expect("Expected Person to be a Dict!");
        assert_eq!(person.len(), 21, "Expected Person to have 21 keys!");
        let id = person
            .get("_id")
            .expect("Expected Person to have key '_id'!");
        assert_eq!(
            id.value_type(),
            ValueType::String,
            "Expected _id to be a String!"
        );
        let age = person
            .get("age")
            .expect("Expected Person to have key 'age'!");
        assert_eq!(
            age.value_type(),
            ValueType::Short,
            "Expected age to be a Short!"
        );
    }

    std::fs::remove_file("10_000people.fleece").expect("Failed to remove file");
}

#[test]
fn shared_keys() {
    let mut encoder = Encoder::new();
    encoder.set_shared_keys(SharedKeys::new());
    encoder.begin_dict().unwrap();
    encoder.write_key("name").expect("Failed to write value!");
    encoder.write_value("John").expect("Failed to write key!");
    encoder
        .write_key("Address Line 1")
        .expect("Failed to write key!");
    encoder
        .write_value("3250 Olcott St")
        .expect("Failed to write value!");

    let scope = encoder.finish_scoped().expect("Failed to create Scope!");
    let shared_keys = scope.shared_keys().expect("Scope should have shared keys!");
    // Expect 1 shared key because "name" is short enough to be a shared key, but "Address Line 1" is not.
    assert_eq!(shared_keys.len(), 1, "Expected 1 shared key!");

    let scoped_value = scope.root().expect("Scope has no root Value!");

    let dict = scoped_value.as_dict().expect("Expected root to be a Dict!");
    let name = dict.get("name").expect("Expected key 'name'!");
    assert_eq!(name.to_str(), "John");
    let address = dict
        .get("Address Line 1")
        .expect("Expected key 'Address Line 1'!");
    assert_eq!(address.to_str(), "3250 Olcott St");
}

#[test]
fn shared_keys_iter() {
    let value = Value::from_bytes(PERSON_ENCODED).unwrap();
    let mut encoder = Encoder::new();
    encoder.set_shared_keys(SharedKeys::new());
    encoder.write_fleece(value).unwrap();
    let scope = encoder.finish_scoped().unwrap();
    let scoped_value = scope.root().unwrap();
    let sk_dict = scoped_value.as_dict().unwrap();

    let non_sk_dict = value.as_dict().unwrap();

    let all_sk_keys: BTreeSet<&str> = sk_dict.into_iter().map(|(key, _)| key).collect();
    let all_non_sk_keys: BTreeSet<&str> = non_sk_dict.into_iter().map(|(key, _)| key).collect();

    assert_eq!(all_sk_keys, all_non_sk_keys);
}
