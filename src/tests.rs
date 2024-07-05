use std::sync::Arc;

use crate::encoder::Encoder;
use crate::sharedkeys::SharedKeys;
use crate::value::ValueType;

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

    // TODO: assert_eq!(res.len(), PERSON_ENCODED.len(), "Expected encoded value to be the same length!");

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
fn shared_keys() {
    let mut encoder = Encoder::new();
    encoder.set_shared_keys(SharedKeys::new());
    encoder.begin_dict();
    encoder.write_key("name").expect("Failed to write key!");
    encoder.write_value("John").expect("Failed to write value!");
    // Keys with spaces cannot be encoded with SharedKeys, so this should be saved as a string
    encoder
        .write_key("Address Line 1")
        .expect("Failed to write key!");
    encoder
        .write_value("3250 Olcott St")
        .expect("Failed to write value!");

    let shared_keys = Arc::new(encoder.shared_keys().unwrap());
    let scope = encoder.finish_scoped().expect("Failed to create Scope");
    assert_eq!(shared_keys.len(), 1, "Expected 1 shared key!");

    let scoped_value = scope.root().expect("Scope has no root Value!");
    let value = scoped_value.value();

    let dict = value.as_dict().expect("Expected value to be a Dict!");
    let name = dict.get("name").expect("Expected Dict to have key 'name'!");
    let address = dict
        .get("Address Line 1")
        .expect("Expected Dict to have key 'Address Line 1'!");
    assert_eq!(
        name.value_type(),
        ValueType::String,
        "Expected name to be a String!"
    );
    assert_eq!(
        address.value_type(),
        ValueType::String,
        "Expected address to be a String!"
    );
    assert_eq!(name.to_str(), "John", "Expected name to be 'John'!");
    assert_eq!(
        address.to_str(),
        "3250 Olcott St",
        "Address did not match expected!"
    );
}

#[test]
fn encode_people_shared_keys() {
    let original = Value::from_bytes(PEOPLE_ENCODED).expect("Failed to decode Fleece");
    let mut encoder = Encoder::new();
    encoder.set_shared_keys(SharedKeys::new());
    encoder.write_fleece(original).expect("Failed to write value!");
    let res = encoder.finish_scoped().expect("Failed to create Scope!");
    let scoped_value = res.root().expect("Scope has no root!");
    decode_people_checks(scoped_value.value());
}

#[test]
fn scope_invalid_root() {
    // Some bytes which are invalid Fleece
    let bytes: Vec<u8> = vec![0x76, 0x61, 0x64, 0x65, 0x72];
    let scope = Scope::new_alloced(bytes, None).expect("Failed to create Scope");
    assert!(scope.root().is_none());
}
