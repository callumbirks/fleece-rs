use crate::encoder::Encoder;
use super::*;

const PERSON_ENCODED: &[u8] = include_bytes!("../1person.fleece");
const PEOPLE_ENCODED: &[u8] = include_bytes!("../1000people.fleece");

#[test]
fn decode_person() {
    let person = Value::from_bytes(PERSON_ENCODED);
    assert!(person.is_some());
    let person = person.unwrap();
    println!("{person}");
    assert!(matches!(person, Value::Dict { .. }));
    let Value::Dict(person_dict) = person else {
        unreachable!()
    };
    let age = person_dict.get(3);
    assert!(age.is_some());
    let (age_key, age_value) = age.unwrap();
    if let Value::String(age_key) = age_key {
        assert_eq!(age_key, "age");
    } else {
        panic!("Expected age key to be a string");
    }
    if let Value::Short(age_value) = age_value {
        assert_eq!(age_value, 30);
    } else {
        panic!("Expected age value to be an int");
    }
}

#[test]
fn encode_person() {
    let mut encoder = Encoder::new();
    encoder.write_key("name").expect("Failed to write key!");
    encoder.write_value("Alice").expect("Failed to write value!");
    let res = encoder.finish();

    let value = Value::from_bytes(&res).expect("Failed to decode encoded value!");
    assert!(matches!(value, Value::Dict { .. }));
    let Value::Dict(dict) = value else {
        unreachable!()
    };
    let (name_key, name_value) = dict.get(0).expect("Failed to get name key!");
    if let Value::String(name_key) = name_key {
        assert_eq!(name_key, "name");
    } else {
        panic!("Expected name key to be a string");
    }
    if let Value::String(name_value) = name_value {
        assert_eq!(name_value, "Alice");
    } else {
        panic!("Expected name value to be a string");
    }
}
