pub use array::Array;
pub use dict::Dict;
use std::fmt::{Display, Formatter};

use crate::raw::{value::RawValue, value::ValueType};

mod array;
mod dict;

pub enum Value<'a> {
    Null,
    Undefined,
    Bool(bool),
    Short(i16),
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
    /// Parse the given data as Fleece data. Returns `None` if the data is not valid Fleece data.
    #[must_use]
    pub fn from_bytes(data: &'a [u8]) -> Option<Value<'a>> {
        let raw_value = RawValue::from_bytes(data)?;
        Value::from_raw(raw_value)
    }

    /// Parse the given data as Fleece data, but with no validation. Much faster than `from_bytes`, but
    /// may cause panics if the data is not valid Fleece data.
    /// # Safety
    /// This should only be used if you are sure that the given data is valid Fleece data.
    #[must_use]
    pub unsafe fn from_bytes_unchecked(data: &'a [u8]) -> Option<Value<'a>> {
        let raw_value = RawValue::from_bytes_unchecked(data);
        Value::from_raw(raw_value)
    }

    fn from_raw(raw_value: &'a RawValue) -> Option<Value<'a>> {
        match raw_value.value_type() {
            ValueType::Null => Some(Value::Null),
            ValueType::Undefined => Some(Value::Undefined),
            ValueType::True => Some(Value::Bool(true)),
            ValueType::False => Some(Value::Bool(false)),
            ValueType::UnsignedInt => {
                Some(Value::Unsigned(raw_value.to_unsigned_int()))
            }
            ValueType::Short | ValueType::Int => Some(Value::Int(raw_value.to_int())),
            ValueType::Float => Some(Value::Float(raw_value.to_float())),
            ValueType::Double32 => Some(Value::Double(raw_value.to_double())),
            ValueType::Double64 => Some(Value::Double(raw_value.to_double())),
            ValueType::String => Some(Value::String(raw_value.to_str())),
            ValueType::Data => Some(Value::Data(raw_value.to_data())),
            ValueType::Dict => Some(Value::Dict(Dict::new(raw_value))),
            ValueType::Array => Some(Value::Array(Array::new(raw_value))),
            // RawValue should never be pointer, as pointers are always de-referenced in `RawValue::from_bytes`
            ValueType::Pointer => None,
        }
    }

    #[must_use] pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    #[must_use] pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Value::Bool(_))
    }

    pub fn is_short(&self) -> bool {
        matches!(self, Value::Short(_))
    }

    pub fn is_unsigned(&self) -> bool {
        matches!(self, Value::Unsigned(_))
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Value::Int(_))
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Value::Float(_))
    }

    pub fn is_double(&self) -> bool {
        matches!(self, Value::Double(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }

    pub fn is_data(&self) -> bool {
        matches!(self, Value::Data(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    pub fn is_dict(&self) -> bool {
        matches!(self, Value::Dict(_))
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_short(&self) -> Option<i16> {
        match self {
            Value::Short(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_unsigned(&self) -> Option<u64> {
        match self {
            Value::Unsigned(u) => Some(*u),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f32> {
        match self {
            Value::Float(flt) => Some(*flt),
            _ => None,
        }
    }

    pub fn as_double(&self) -> Option<f64> {
        match self {
            Value::Double(dbl) => Some(*dbl),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(*s),
            _ => None,
        }
    }

    pub fn as_data(&self) -> Option<&[u8]> {
        match self {
            Value::Data(d) => Some(*d),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Array<'a>> {
        match self {
            Value::Array(arr) => Some(arr),
            _ => None,
        }
    }

    pub fn as_dict(&self) -> Option<&Dict<'a>> {
        match self {
            Value::Dict(dict) => Some(dict),
            _ => None,
        }
    }
}

impl Display for Value<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Undefined => write!(f, "undefined"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Short(i) => write!(f, "{i}"),
            Value::Unsigned(u) => write!(f, "{u}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(flt) => write!(f, "{flt}"),
            Value::Double(dbl) => write!(f, "{dbl}"),
            Value::String(s) => write!(f, "{s}"),
            Value::Data(d) => write!(f, "{d:?}"),
            Value::Array(arr) => {
                write!(f, "Array[")?;
                for val in arr {
                    write!(f, "{val}, ")?;
                }
                write!(f, "]")
            }
            Value::Dict(dict) => {
                writeln!(f, "Dict[")?;
                for (key, val) in dict {
                    writeln!(f, "{key} : {val},")?;
                }
                writeln!(f, "]")
            }
        }
    }
}
