use core::fmt;

pub type Result<T> = core::result::Result<T, EncodeError>;

#[derive(Debug)]
pub enum EncodeError {
    ArrayNotOpen,
    DictNotOpen,
    DictWaitingForValue,
    DictWaitingForKey,
    CollectionNotOpen,
    PointerTooLarge,
    MultiTopLevelCollection,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::ArrayNotOpen => write!(f, "No open array"),
            EncodeError::DictNotOpen => write!(f, "No open dictionary"),
            EncodeError::DictWaitingForValue => write!(
                f,
                "Cannot write another key until the previous key has a value written to it"
            ),
            EncodeError::DictWaitingForKey => {
                write!(f, "Cannot write a value until a key has been written")
            }
            EncodeError::CollectionNotOpen => write!(f, "No open collection to write a value to"),
            EncodeError::PointerTooLarge => write!(f, "Value Pointer too large to be encoded"),
            EncodeError::MultiTopLevelCollection => {
                write!(f, "Multiple top level collections are not allowed")
            }
        }
    }
}
