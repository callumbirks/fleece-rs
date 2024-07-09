use crate::Value;
use std::backtrace::Backtrace;
use thiserror::Error;
use crate::value::ValueType;

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
    #[error("Pointer with offset {offset} target {target:#x} outside of source data (start: {data_start:#x})")]
    PointerTargetOutOfBounds {
        data_start: usize,
        target: usize,
        offset: usize,
    },
    #[error("Array with width {width} and {count} elements exceeded the available {available_size} bytes")]
    ArrayOutOfBounds {
        count: usize,
        width: usize,
        available_size: usize,
        bytes: Box<[u8]>,
    },
    #[error("Value with type {value_type:?} which requires {required_size} bytes exceeded the available {available_size} bytes")]
    ValueOutOfBounds {
        value_type: ValueType,
        required_size: usize,
        available_size: usize,
    },
}
