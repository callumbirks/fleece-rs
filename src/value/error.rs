use core::fmt;

use crate::value::ValueType;

pub type Result<T> = core::result::Result<T, DecodeError>;

#[derive(Debug)]
pub enum DecodeError {
    InputIncorrectlySized,
    RootNotPointer,
    PointerTooSmall {
        actual: usize,
        expected: usize,
    },
    PointerOffsetZero,
    IsNotDict,
    IsNotArray,
    PointerTargetOutOfBounds {
        data_start: usize,
        target: usize,
        offset: u32,
    },
    ArrayOutOfBounds {
        count: usize,
        width: usize,
        available_size: usize,
    },
    ValueOutOfBounds {
        value_type: ValueType,
        required_size: usize,
        available_size: usize,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::InputIncorrectlySized => write!(f, "Incorrectly sized input data"),
            DecodeError::RootNotPointer => write!(f, "Root value is not a pointer"),
            DecodeError::PointerTooSmall { actual, expected } => write!(
                f,
                "Pointer expected to be {expected} bytes, but was {actual} bytes"
            ),
            DecodeError::PointerOffsetZero => write!(f, "Pointer offset of 0"),
            DecodeError::IsNotDict => write!(f, "Value is not a dictionary"),
            DecodeError::IsNotArray => write!(f, "Value is not an array"),
            DecodeError::PointerTargetOutOfBounds {
                data_start,
                target,
                offset,
            } => write!(f, "Pointer with offset {offset} target {target:#x} outside of source data (start: {data_start:#x})"),
            DecodeError::ArrayOutOfBounds {
                count,
                width,
                available_size,
            } => write!(f, "Array with width {width} and {count} elements exceeded the available {available_size} bytes"),
            DecodeError::ValueOutOfBounds {
                value_type,
                required_size,
                available_size,
            } => write!(f, "Value with type {value_type:?} which requires {required_size} bytes exceeded the available {available_size} bytes"),
        }
    }
}
