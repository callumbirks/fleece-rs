use mutable::{MutableArray, MutableDict};

use crate::encoder::Encoder;
use crate::value::{varint, ValueType};
use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;

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
    let mut encoder = Encoder::new();
    let array = original.as_array().unwrap();
    encoder.begin_array(10_000).unwrap();
    for _ in 0..10 {
        for value in array {
            encoder.write_fleece(value).unwrap();
        }
    }
    encoder.end_array().unwrap();
    let vec = encoder.finish();
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("10_000people.fleece")
        .unwrap();
    file.write_all(&vec).unwrap();
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

    let scope = encoder.finish_scoped();
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
    let scope = encoder.finish_scoped();
    let scoped_value = scope.root().unwrap();
    let sk_dict = scoped_value.as_dict().unwrap();

    let non_sk_dict = value.as_dict().unwrap();

    let all_sk_keys: BTreeSet<&str> = sk_dict.into_iter().map(|(key, _)| key).collect();
    let all_non_sk_keys: BTreeSet<&str> = non_sk_dict.into_iter().map(|(key, _)| key).collect();

    assert_eq!(all_sk_keys, all_non_sk_keys);
}

#[test]
fn alloced_value() {
    let value = Value::clone_from_bytes(PERSON_ENCODED).unwrap();
    decode_person_checks(&value);
    // Sanity check
    assert_eq!(Arc::strong_count(&value.buf), 1);
}

#[test]
fn mutable_dict() {
    let dict = Value::clone_from_bytes(PERSON_ENCODED)
        .unwrap()
        .to_dict()
        .unwrap();
    let mut dict = MutableDict::from(dict);
    assert_eq!(dict["age"].to_short(), 30);
    dict.insert("age", 52);
    assert_eq!(dict["age"].to_short(), 52);
    dict.remove("age");
    assert!(!dict.contains_key("age"));

    let new_dict = dict.clone();
    assert!(!new_dict.contains_key("age"));
    dict.insert("age", 28);
    assert_eq!(dict["age"].to_short(), 28);
}

#[test]
fn mutable_array() {
    let strings = ["Zero", "One", "Two", "Three", "Four", "Five", "Six"];

    let mut encoder = Encoder::new();
    encoder.begin_array(7).unwrap();
    for string in strings {
        encoder.write_value(string).unwrap();
    }
    encoder.end_array().unwrap();
    let array = encoder.finish_value().to_array().unwrap();

    let mut array = MutableArray::from(array);

    for (i, string) in strings.iter().enumerate() {
        assert_eq!(array[i].to_str(), *string);
    }

    array.push("Seven");
    assert!(array[array.len() - 1].to_str() == "Seven");
    array.remove(2);
    assert_eq!(array[2].to_str(), "Three");
    assert_eq!(array.len(), 7);

    let new_array = array.clone();
    assert_eq!(new_array[2].to_str(), "Three");
    assert_eq!(new_array[6].to_str(), "Seven");
}

#[test]
fn nested_mutable_dict() {
    let mut encoder = Encoder::new();
    encoder.begin_dict().unwrap();
    encoder.write_key("name").unwrap();
    encoder.write_value("Jeff").unwrap();
    encoder.write_key("contact").unwrap();
    encoder.begin_dict().unwrap();
    encoder.write_key("email").unwrap();
    encoder.write_value("contact@jeffbaggins.com").unwrap();
    encoder.write_key("phone_number").unwrap();
    encoder.write_value("+1 234 56789").unwrap();
    encoder.end_dict().unwrap();
    encoder.end_dict().unwrap();
    let dict = encoder.finish_value().to_dict().unwrap();

    let mut dict = MutableDict::from(dict);
    let contact_dict = dict.get_dict("contact").unwrap();
    assert_eq!(contact_dict["email"].to_str(), "contact@jeffbaggins.com");
    assert_eq!(contact_dict["phone_number"].to_str(), "+1 234 56789");

    let contact_dict = dict.get_dict_mut("contact").unwrap();
    contact_dict.insert("Address", "3250 Olcott St");
    assert_eq!(contact_dict["Address"].to_str(), "3250 Olcott St");

    let mut email = MutableArray::new();
    email.push_fleece(&contact_dict["email"]);
    email.push("jeff.baggins@example.com");
    dict.insert_array("email", email);

    let email = dict.get_array("email").unwrap();
    assert_eq!(email[0].to_str(), "contact@jeffbaggins.com");
    assert_eq!(email[1].to_str(), "jeff.baggins@example.com");
}

#[test]
fn nested_mutable_array() {
    let mut encoder = Encoder::new();
    encoder.begin_array(10).unwrap();
    encoder.write_value(&23).unwrap();
    encoder.begin_dict().unwrap();
    encoder.write_key("name").unwrap();
    encoder.write_value("Jeff").unwrap();
    encoder.write_key("age").unwrap();
    encoder.write_value(&35).unwrap();
    encoder.end_dict().unwrap();
    encoder.end_array().unwrap();
    let array = encoder.finish_value().to_array().unwrap();

    println!("NESTED MUTABLE ARRAY: {:?}", &array);

    let mut array = MutableArray::from(array);
    let profile_dict = array.get_dict(1).unwrap();
    assert_eq!(profile_dict["name"].to_str(), "Jeff");
    assert_eq!(profile_dict["age"].to_short(), 35);

    let profile_dict = array.get_dict_mut(1).unwrap();
    profile_dict.insert("Address", "3250 Olcott St");
    assert_eq!(profile_dict["Address"].to_str(), "3250 Olcott St");
}
