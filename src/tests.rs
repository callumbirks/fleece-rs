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
    if let Value::Int(age_value) = age_value {
        assert_eq!(age_value, 30);
    } else {
        panic!("Expected age value to be an int");
    }
}

#[test]
fn encode_person() {}
