use thiserror::Error;

pub type Result<T> = std::result::Result<T, EncodeError>;

#[derive(Debug, Error)]
pub enum EncodeError {
    #[error("No open array")]
    ArrayNotOpen,
    #[error("No open dictionary")]
    DictNotOpen,
    #[error("Cannot write another key until the previous key has a value written to it")]
    DictWaitingForValue,
    #[error("Cannot write a value until a key has been written")]
    DictWaitingForKey,
    #[error("No open collection to write value to")]
    CollectionNotOpen,
    #[error("IOError while writing value")]
    Io {
        #[from]
        source: std::io::Error,
    },
    #[error("Value Pointer too large to be encoded")]
    PointerTooLarge,
    #[error("Multiple top level collections are not allowed")]
    MultiTopLevelCollection,
}
