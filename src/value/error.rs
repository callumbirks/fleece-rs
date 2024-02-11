use thiserror::Error;
use crate::Value;

pub type Result<T> = std::result::Result<T, DecodeError>;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("Incorrectly sized input data")]
    InputIncorrectlySized,
    #[error("Root value is not a pointer")]
    RootNotPointer,
    #[error("Pointer expected to be {expected} bytes, but was {actual} bytes")]
    PointerTooSmall { actual: usize, expected: usize },
    #[error("A pointer offset was 0")]
    PointerOffsetZero,
    #[error("Non-external pointer target outside of source data")]
    PointerTargetOutOfBounds,
    #[error("Array with width {width} and {count} elements exceeded the available {available_size} bytes")]
    ArrayOutOfBounds {
        count: usize,
        width: usize,
        available_size: usize,
    },
    #[error("Value {value:?} which requires {required_size} bytes exceeded the available {available_size} bytes")]
    ValueOutOfBounds {
        value: Box<Value>,
        required_size: usize,
        available_size: usize,
    },
}
