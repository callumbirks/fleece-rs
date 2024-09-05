use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Eq, PartialEq, Hash, Debug, Clone)]
enum Class {
    Maths,
    English,
    Science(ScienceClass),
    Other(String),
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Hash, Debug, Clone)]
enum ScienceClass {
    Physics,
    Biology,
    Chemistry,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
enum Grade {
    A,
    B,
    C,
    D,
    E,
    F,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
enum GamePlatform {
    PC,
    PlayStation,
    Xbox,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
enum Favourite {
    Song {
        artist: String,
        name: String,
    },
    Movie(String),
    Game {
        name: String,
        platform: GamePlatform,
    },
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct Student {
    id: u64,
    name: String,
    age: u16,
    favourite_class: Option<Class>,
    favourites: Vec<Favourite>,
    lucky_floats: Option<(f32, f32, f32)>,
}

#[derive(Serialize, Deserialize)]
struct MyUnitStruct;

#[cfg(feature = "serde")]
#[test]
fn serde() {
    let students = vec![
        Student {
            id: 123_456,
            name: "Jens".to_string(),
            age: 17,
            favourite_class: Some(Class::Maths),
            favourites: vec![
                Favourite::Song {
                    artist: "Queen".to_string(),
                    name: "We Will Rock You".to_string(),
                },
                Favourite::Game {
                    platform: GamePlatform::PC,
                    name: "Doom".to_string(),
                },
            ],
            lucky_floats: None,
        },
        Student {
            id: 946,
            name: "Bork".to_string(),
            age: 16,
            favourite_class: Some(Class::Other("Computer Science".to_string())),
            favourites: vec![
                Favourite::Movie("Rogue One: A Star Wars Story".to_string()),
                Favourite::Song {
                    artist: "Linkin Park".to_string(),
                    name: "From the Inside".to_string(),
                },
            ],
            lucky_floats: Some((7.689, -31.48501, 56_587_462.21)),
        },
    ];

    let bytes = fleece::to_bytes(&students).expect("Error serializing");
    let de_students: Vec<Student> = fleece::from_bytes(&bytes).expect("Error deserializing");

    assert_eq!(students, de_students);

    let class = Class::Maths;
    let bytes = fleece::to_bytes(&class).expect("Error serializing");
    let de_class: Class = fleece::from_bytes(&bytes).expect("Error deserializing");
    assert_eq!(class, de_class);

    assert!(matches!(
        fleece::to_bytes("hihi").expect_err("Should throw `SerializeError::ValueNotCollection`"),
        Error::Serialize(error::SerializeError::ValueNotCollection)
    ));

    assert!(matches!(
        fleece::to_bytes(MyUnitStruct)
            .expect_err("Should throw `SerializeError::ValueNotCollection`"),
        Error::Serialize(error::SerializeError::ValueNotCollection)
    ));

    assert!(fleece::to_bytes(Class::Maths).is_ok());
}
