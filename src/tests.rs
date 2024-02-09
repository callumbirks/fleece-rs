use super::*;
use crate::encoder::Encoder;
use crate::value::ValueType;

const PERSON_ENCODED: &[u8] = include_bytes!("../1person.fleece");
const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");

#[test]
fn decode_person() {
    let person = Value::from_bytes(PERSON_ENCODED);
    assert!(person.is_some());
    let person = person.unwrap();
    println!("{person}");
    assert!(person.value_type() == ValueType::Dict);
    let person_dict = person.as_dict().unwrap();
    let age = person_dict.get("age");
    assert!(age.is_some());
    let age = age.unwrap();
    assert!(age.value_type() == ValueType::Short);
    assert_eq!(age.to_short(), 30);
}

#[test]
fn encode_person() {
    let mut encoder = Encoder::new();
    encoder.write_key("name").expect("Failed to write key!");
    encoder
        .write_value("Alice")
        .expect("Failed to write value!");
    let res = encoder.finish();

    let value = Value::from_bytes(&res).expect("Failed to decode encoded value!");
    assert!(value.value_type() == ValueType::Dict);
    let dict = value.as_dict().unwrap();
    let name = dict.get("name").expect("Failed to get name key!");
    assert_eq!(name.to_str(), "Alice");
}
