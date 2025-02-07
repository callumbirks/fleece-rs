pub mod array;
pub mod dict;

use alloc::{boxed::Box, collections::BTreeSet, sync::Arc, vec::Vec};

pub use array::MutableArray;
pub use dict::MutableDict;

use crate::{
    encoder::{Encodable, NullValue, UndefinedValue},
    value, Value,
};

const INLINE_CAPACITY: usize = 15;

#[derive(Debug)]
enum ValueSlot {
    Inline([u8; INLINE_CAPACITY]),
    Pointer(Box<Value>),
    MutableArray(Box<MutableArray>),
    MutableDict(Box<MutableDict>),
}

impl ValueSlot {
    pub fn new<T>(value: T) -> Self
    where
        T: Encodable,
    {
        if value.fleece_size() <= INLINE_CAPACITY {
            let mut buf = [0u8; INLINE_CAPACITY];
            value.write_fleece_to(&mut buf, false);
            Self::Inline(buf)
        } else {
            let mut buf: Box<[u8]> = core::iter::repeat(0u8).take(value.fleece_size()).collect();
            value.write_fleece_to(&mut buf, false);
            Self::Pointer(unsafe { core::mem::transmute(buf) })
        }
    }

    pub fn new_from_fleece(value: &Value, is_wide: bool) -> Self {
        match value.value_type() {
            crate::ValueType::Null => Self::new(NullValue),
            crate::ValueType::Undefined => Self::new(UndefinedValue),
            crate::ValueType::False => Self::new(false),
            crate::ValueType::True => Self::new(true),
            crate::ValueType::Short => Self::new(value.to_short()),
            crate::ValueType::Int => Self::new(value.to_int()),
            crate::ValueType::UnsignedInt => Self::new(value.to_unsigned_int()),
            crate::ValueType::Float => Self::new(value.to_float()),
            crate::ValueType::Double32 | crate::ValueType::Double64 => Self::new(value.to_double()),
            crate::ValueType::String => Self::new(value.to_str()),
            crate::ValueType::Data => Self::new(value.to_data()),
            crate::ValueType::Array => {
                Self::new_array(MutableArray::clone_from(value.as_array().unwrap()))
            }
            crate::ValueType::Dict => {
                Self::new_dict(MutableDict::clone_from(value.as_dict().unwrap()))
            }
            crate::ValueType::Pointer => Self::new_from_fleece(
                unsafe {
                    crate::value::pointer::Pointer::from_value(value).deref_unchecked(is_wide)
                },
                false,
            ),
        }
    }

    pub fn new_dict(dict: MutableDict) -> Self {
        Self::MutableDict(Box::new(dict))
    }

    pub fn new_array(array: MutableArray) -> Self {
        Self::MutableArray(Box::new(array))
    }

    pub fn is_value(&self) -> bool {
        matches!(self, ValueSlot::Inline(_) | ValueSlot::Pointer(_))
    }

    #[inline]
    pub fn is_array(&self) -> bool {
        matches!(self, ValueSlot::MutableArray(_))
    }

    #[inline]
    pub fn is_dict(&self) -> bool {
        matches!(self, ValueSlot::MutableDict(_))
    }

    pub fn value(&self) -> Option<&Value> {
        match self {
            ValueSlot::Pointer(vp) => Some(vp.as_ref()),
            ValueSlot::Inline(inline) => {
                Some(unsafe { &*(core::ptr::from_ref::<[u8]>(inline.as_slice()) as *const Value) })
            }
            ValueSlot::MutableArray(_) | ValueSlot::MutableDict(_) => None,
        }
    }

    pub fn array(&self) -> Option<&MutableArray> {
        match self {
            ValueSlot::MutableArray(arr) => Some(arr.as_ref()),
            _ => None,
        }
    }

    pub fn dict(&self) -> Option<&MutableDict> {
        match self {
            ValueSlot::MutableDict(dict) => Some(dict.as_ref()),
            _ => None,
        }
    }

    pub fn array_mut(&mut self) -> Option<&mut MutableArray> {
        match self {
            ValueSlot::MutableArray(arr) => Some(arr.as_mut()),
            _ => None,
        }
    }

    pub fn dict_mut(&mut self) -> Option<&mut MutableDict> {
        match self {
            ValueSlot::MutableDict(dict) => Some(dict.as_mut()),
            _ => None,
        }
    }

    #[cold]
    #[inline(never)]
    fn _undefined() -> Self {
        let mut i = [0u8; INLINE_CAPACITY];
        i[0] = value::constants::UNDEFINED[0];
        i[1] = value::constants::UNDEFINED[1];
        Self::Inline(i)
    }

    #[cold]
    #[inline(never)]
    fn pointer_overflow_panic() -> ! {
        panic!("Overflow for Value len in `ValueSlot::new_pointer`");
    }
}

impl Clone for ValueSlot {
    fn clone(&self) -> Self {
        match self {
            ValueSlot::Inline(i) => ValueSlot::Inline(*i),
            ValueSlot::Pointer(p) => {
                let mut buf: Box<[u8]> = core::iter::repeat(0u8).take(p.len()).collect();
                buf.copy_from_slice(&p.bytes);
                ValueSlot::Pointer(unsafe { core::mem::transmute(buf) })
            }
            ValueSlot::MutableArray(arr) => ValueSlot::MutableArray(arr.clone()),
            ValueSlot::MutableDict(dict) => ValueSlot::MutableDict(dict.clone()),
        }
    }
}
