pub mod array;
pub mod dict;

use alloc::{boxed::Box, collections::BTreeSet, sync::Arc};

pub use array::MutableArray;
pub use dict::MutableDict;

use crate::{
    alloced::AllocedValue,
    encoder::{Encodable, NullValue, UndefinedValue},
    Value,
};

const INLINE_CAPACITY: usize = 15;

#[derive(Debug, Clone)]
enum ValueSlot {
    Empty,
    Pointer(ValuePointer),
    Inline([u8; INLINE_CAPACITY]),
    MutableArray(Box<MutableArray>),
    MutableDict(Box<MutableDict>),
}

impl ValueSlot {
    pub fn new_inline<T>(value: T) -> Self
    where
        T: Encodable,
    {
        struct SizeCheck;
        impl SizeCheck {
            const CHECK: () = assert!(size_of::<ValueSlot>() == 16);
        }
        let _ = SizeCheck::CHECK;

        debug_assert!(value.fleece_size() <= INLINE_CAPACITY);
        let mut buf = [0u8; INLINE_CAPACITY];
        value.write_fleece_to(&mut buf, false);
        Self::Inline(buf)
    }

    pub fn new_pointer(value: &Value) -> Self {
        let Ok(len) = u32::try_from(value.len()) else {
            #[cfg(debug_assertions)]
            panic!("Overflow for Value len in `ValueSlot::new_pointer`");
            #[cfg(not(debug_assertions))]
            return Self::Empty;
        };
        Self::Pointer(ValuePointer {
            ptr: value.bytes.as_ptr(),
            len,
        })
    }

    #[inline]
    pub fn is_pointer(&self) -> bool {
        matches!(self, ValueSlot::Pointer(_))
    }

    #[inline]
    pub fn is_inline(&self) -> bool {
        matches!(self, ValueSlot::Inline(_))
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        matches!(self, ValueSlot::Empty)
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
            ValueSlot::Empty => None,
            ValueSlot::Pointer(vp) => {
                let slice = core::ptr::slice_from_raw_parts(vp.ptr, vp.len as usize);
                Some(unsafe { core::mem::transmute(&*slice) })
            }
            ValueSlot::Inline(inline) => Some(unsafe { core::mem::transmute(inline.as_slice()) }),
            ValueSlot::MutableArray(_) => None,
            ValueSlot::MutableDict(_) => None,
        }
    }

    pub fn pointer(&self) -> Option<*const Value> {
        match self {
            ValueSlot::Pointer(vp) => {
                let slice = core::ptr::slice_from_raw_parts(vp.ptr, vp.len as usize);
                Some(unsafe { core::mem::transmute(slice) })
            }
            _ => None,
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
}

fn encode<T>(allocated_values: &mut BTreeSet<AllocedValue>, value: T) -> ValueSlot
where
    T: Encodable,
{
    if value.fleece_size() <= INLINE_CAPACITY {
        ValueSlot::new_inline(value)
    } else {
        let mut buf: Box<[u8]> = core::iter::repeat(0).take(value.fleece_size()).collect();
        value.write_fleece_to(&mut buf, false).expect(
            "Encoding should not fail because we allocated the buffer with the needed size.",
        );
        let buf: Arc<[u8]> = Arc::from(buf);
        let pointer = buf.as_ref() as *const [u8] as *const Value;
        let alloced = AllocedValue {
            buf,
            value: pointer,
        };
        allocated_values.insert(alloced);
        let alloced: &AllocedValue = allocated_values.last().unwrap();
        ValueSlot::new_pointer(alloced)
    }
}

fn encode_fleece(
    allocated_values: &mut BTreeSet<AllocedValue>,
    value: &Value,
    is_wide: bool,
) -> ValueSlot {
    match value.value_type() {
        crate::ValueType::Null => encode(allocated_values, NullValue),
        crate::ValueType::Undefined => encode(allocated_values, UndefinedValue),
        crate::ValueType::False => encode(allocated_values, false),
        crate::ValueType::True => encode(allocated_values, true),
        crate::ValueType::Short => encode(allocated_values, value.to_short()),
        crate::ValueType::Int => encode(allocated_values, value.to_int()),
        crate::ValueType::UnsignedInt => encode(allocated_values, value.to_unsigned_int()),
        crate::ValueType::Float => encode(allocated_values, value.to_float()),
        crate::ValueType::Double32 | crate::ValueType::Double64 => {
            encode(allocated_values, &value.to_double())
        }
        crate::ValueType::String => encode(allocated_values, value.to_str()),
        crate::ValueType::Data => encode(allocated_values, value.to_data()),
        crate::ValueType::Array => {
            let source = value.as_array().unwrap();
            ValueSlot::MutableArray(Box::new(MutableArray::clone_from(source)))
        }
        crate::ValueType::Dict => {
            let source = value.as_dict().unwrap();
            ValueSlot::MutableDict(Box::new(MutableDict::clone_from(source)))
        }
        crate::ValueType::Pointer => encode_fleece(
            allocated_values,
            unsafe { crate::value::pointer::Pointer::from_value(value).deref_unchecked(is_wide) },
            is_wide,
        ),
    }
}

#[derive(Clone)]
struct ValueSlotInline {
    val: [u8; INLINE_CAPACITY],
}

#[repr(u8)]
enum InlineValue {
    Value([u8; INLINE_CAPACITY]),
    MutableArray(Box<MutableArray>),
    MutableDict(Box<MutableDict>),
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum InlineType {
    Value,
    MutableArray,
    MutableDict,
}

#[derive(Debug, Clone, Copy)]
#[repr(packed(4))]
struct ValuePointer {
    ptr: *const u8,
    len: u32,
}
