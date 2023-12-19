pub use array::Array;
pub use dict::Dict;
use std::fmt::{Display, Formatter};

use crate::raw::{RawValue, ValueType};

mod array;
mod dict;

pub enum Value<'a> {
    Null,
    Undefined,
    Bool(bool),
    Unsigned(u64),
    Int(i64),
    Float(f32),
    Double(f64),
    String(&'a str),
    Data(&'a [u8]),
    Array(Array<'a>),
    Dict(Dict<'a>),
}

impl<'a> Value<'a> {
    pub fn from_data(data: &'a [u8]) -> Option<Value<'a>> {
        let raw_value = RawValue::from_data(data)?;
        Value::from_raw(raw_value)
    }

    pub unsafe fn from_data_unchecked(data: &'a [u8]) -> Option<Value<'a>> {
        let raw_value = RawValue::from_data_unchecked(data);
        Value::from_raw(raw_value)
    }

    pub(crate) fn from_raw(raw_value: &'a RawValue) -> Option<Value<'a>> {
        match raw_value.value_type() {
            ValueType::Null => Some(Value::Null),
            ValueType::Undefined => Some(Value::Undefined),
            ValueType::True => Some(Value::Bool(true)),
            ValueType::False => Some(Value::Bool(false)),
            ValueType::UnsignedShort | ValueType::UnsignedInt => {
                Some(Value::Unsigned(raw_value.as_unsigned_int()))
            }
            ValueType::Short | ValueType::Int => Some(Value::Int(raw_value.as_int())),
            ValueType::Float => Some(Value::Float(raw_value.as_float())),
            ValueType::Double => Some(Value::Double(raw_value.as_double())),
            ValueType::String => Some(Value::String(raw_value.as_str())),
            ValueType::Data => Some(Value::Data(raw_value.as_data())),
            ValueType::Dict => Some(Value::Dict(Dict::new(raw_value))),
            ValueType::Array => Some(Value::Array(Array::new(raw_value))),
            // RawValue should never be pointer, as pointers are always dereferenced in from_data
            ValueType::Pointer => None,
        }
    }
}

impl Display for Value<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Undefined => write!(f, "undefined"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Unsigned(u) => write!(f, "{}", u),
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(flt) => write!(f, "{}", flt),
            Value::Double(dbl) => write!(f, "{}", dbl),
            Value::String(s) => write!(f, "{}", s),
            Value::Data(d) => write!(f, "{:?}", d),
            Value::Array(arr) => {
                write!(f, "Array[")?;
                for val in arr.into_iter() {
                    write!(f, "{}, ", val)?;
                }
                write!(f, "]")
            }
            Value::Dict(dict) => {
                writeln!(f, "Dict[")?;
                for (key, val) in dict.into_iter() {
                    writeln!(f, "{} : {},", key, val)?;
                }
                writeln!(f, "]")
            }
        }
    }
}
