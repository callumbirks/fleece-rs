#![allow(clippy::transmute_ptr_to_ptr)]

pub(crate) mod array;
pub(crate) mod dict;
pub(crate) mod pointer;
mod sized;
pub(crate) mod varint;

mod error;

pub use array::Array;
pub use dict::Dict;
pub use sized::SizedValue;

use crate::alloced::AllocedValue;
pub use error::DecodeError;
use error::Result;
use pointer::Pointer;
use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};

#[repr(transparent)]
pub struct Value {
    pub(super) bytes: [u8],
}

#[derive(PartialEq, Eq)]
pub enum ValueType {
    Null,
    Undefined,
    False,
    True,
    Short,
    Int,
    UnsignedInt,
    Float,
    // Double32 is encoded as a 32-bit float, but should be decoded into a 64-bit float. This avoids precision loss in
    // cases where the Encoder encodes a 64-bit float as a 32-bit float because the float is representable in 32 bits.
    // See https://github.com/couchbase/fleece/issues/206
    Double32,
    Double64,
    String,
    Data,
    Array,
    Dict,
    Pointer,
}

pub(crate) mod tag {
    pub const SHORT: u8 = 0x00;
    pub const INT: u8 = 0x10;
    pub const FLOAT: u8 = 0x20;
    pub const SPECIAL: u8 = 0x30;
    pub const STRING: u8 = 0x40;
    pub const DATA: u8 = 0x50;
    pub const ARRAY: u8 = 0x60;
    pub const DICT: u8 = 0x70;
    // Pointers are 0x80 to 0xF0
    pub const POINTER: u8 = 0x80;
}

pub(crate) mod special_tag {
    pub const NULL: u8 = 0x00;
    pub const UNDEFINED: u8 = 0x0C;
    pub const FALSE: u8 = 0x04;
    pub const TRUE: u8 = 0x08;
}

pub(crate) mod extra_flags {
    pub const UNSIGNED_INT: u8 = 0x08;
    pub const DOUBLE_ENCODED: u8 = 0x08;
    pub const DOUBLE_DECODED: u8 = 0x04;
}

impl ValueType {
    #[inline]
    #[must_use]
    pub fn from_byte(byte: u8) -> Self {
        match byte & 0xF0 {
            // Some types have extra info in the lower 4 bits
            tag::SPECIAL => match byte & 0x0F {
                special_tag::UNDEFINED => ValueType::Undefined,
                special_tag::FALSE => ValueType::False,
                special_tag::TRUE => ValueType::True,
                _ => ValueType::Null,
            },
            tag::SHORT => ValueType::Short,
            // 0x08 bit set means int is unsigned.
            tag::INT => match byte & extra_flags::UNSIGNED_INT {
                0x00 => ValueType::Int,
                _ => ValueType::UnsignedInt,
            },
            // For floats, the 5th bit signifies 32 / 64-bit (0 or 1). The 6th bit signifies if this should be decoded into a
            // 32-bit or 64-bit value (0 or 1). This can avoid precision loss in some cases.
            // See https://github.com/couchbase/fleece/issues/206
            tag::FLOAT => {
                match byte & (extra_flags::DOUBLE_ENCODED | extra_flags::DOUBLE_DECODED) {
                    0x00 => ValueType::Float,
                    extra_flags::DOUBLE_DECODED => ValueType::Double32,
                    _ => ValueType::Double64,
                }
            }
            tag::STRING => ValueType::String,
            tag::DATA => ValueType::Data,
            tag::ARRAY => ValueType::Array,
            tag::DICT => ValueType::Dict,
            // Pointers are 0x80 to 0xF0, so we don't compare directly to Tag::Pointer
            _ => ValueType::Pointer,
        }
    }
}

pub mod constants {
    use super::{special_tag, tag};

    pub const TRUE: [u8; 2] = [tag::SPECIAL | special_tag::TRUE, 0x00];
    pub const FALSE: [u8; 2] = [tag::SPECIAL | special_tag::FALSE, 0x00];
    pub const NULL: [u8; 2] = [tag::SPECIAL | special_tag::NULL, 0x00];
    pub const UNDEFINED: [u8; 2] = [tag::SPECIAL | special_tag::UNDEFINED, 0x00];
}

// API
impl Value {
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Find and validate Fleece data in the given data. It will return a reference to the root
    /// value. The root value will usually be a [Dict].
    /// ## Errors
    /// If the data given is not valid Fleece data
    pub fn from_bytes(data: &[u8]) -> Result<&Self> {
        let root = Self::_find_root(data)?;
        let data_start = data.as_ptr();
        let data_end = unsafe { data_start.add(data.len()) };
        // wide parameter doesn't matter here, as it's only used for pointers, and find_root will
        // never return a pointer.
        root._validate::<false>(false, data_start, data_end)?;
        Ok(root)
    }

    /// Like [`Value::from_bytes`], but doesn't do any validation, so it should only be used on data that
    /// you already know to be valid Fleece.
    /// If you call this on invalid Fleece data, it will probably panic.
    /// The performance uplift of this function is great, but must be used carefully.
    /// # Safety
    /// The caller should ensure the data is valid Fleece data.
    /// # Panics
    /// If the data is invalid Fleece data.
    #[must_use]
    pub unsafe fn from_bytes_unchecked(data: &[u8]) -> &Self {
        // Root is 2 bytes at the end of the data
        let root = &data[(data.len() - 2)..];
        let root: &Value = std::mem::transmute(root);
        if root.value_type() == ValueType::Pointer {
            return Pointer::from_value(root).deref_unchecked(false);
        } else if data.len() == 2 {
            return root;
        }
        panic!("Invalid data");
    }

    /// The same as [`Value::from_bytes`], but returning an `AllocedValue`, so the caller doesn't
    /// need to worry about keeping the original bytes in scope.
    /// This method clones the passed in data.
    /// # Errors
    /// See [`Value::from_bytes`]
    pub fn clone_from_bytes(data: &[u8]) -> Result<AllocedValue> {
        let mut alloced = unsafe { AllocedValue::new_dangling(data) };
        let value = Value::from_bytes(&alloced.buf)?;
        alloced.value = std::ptr::from_ref(value);
        Ok(alloced)
    }

    /// The same as [`Value::from_bytes_unchecked`], but returning an `AllocedValue`, so the caller
    /// doesn't need to worry about keeping the original bytes in scope.
    /// This method clones the passed in data.
    /// # Safety
    /// See [`Value::from_bytes_unchecked`]
    #[must_use]
    pub unsafe fn clone_from_bytes_unchecked(data: &[u8]) -> AllocedValue {
        let mut alloced = unsafe { AllocedValue::new_dangling(data) };
        let value = Value::from_bytes_unchecked(&alloced.buf);
        alloced.value = std::ptr::from_ref(value);
        alloced
    }

    // Will cause a panic if bytes is empty
    #[inline]
    #[must_use]
    pub fn value_type(&self) -> ValueType {
        ValueType::from_byte(self.bytes[0])
    }
}

// Into Conversions
impl Value {
    // False is false, Numbers not equal to 0 are false, everything else is true
    #[must_use]
    pub fn to_bool(&self) -> bool {
        match self.value_type() {
            ValueType::False => false,
            ValueType::Short
            | ValueType::Int
            | ValueType::Float
            | ValueType::Double32
            | ValueType::Double64 => self.to_int() != 0,
            _ => true,
        }
    }

    #[allow(clippy::match_same_arms)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    #[must_use]
    pub fn to_short(&self) -> i16 {
        match self.value_type() {
            ValueType::True => 1,
            ValueType::False => 0,
            // Short is always negative, so sign extend it.
            ValueType::Short => {
                let i = self._get_short();
                if i & 0x0800 != 0 {
                    (i | 0xF000) as i16
                } else {
                    i as i16
                }
            }
            ValueType::Int | ValueType::UnsignedInt => self.to_int() as i16,
            ValueType::Float | ValueType::Double32 | ValueType::Double64 => self.to_double() as i16,
            _ => 0,
        }
    }

    #[allow(clippy::match_same_arms)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    #[allow(clippy::cast_sign_loss)]
    #[must_use]
    pub fn to_unsigned_short(&self) -> u16 {
        match self.value_type() {
            ValueType::True => 1,
            ValueType::False => 0,
            ValueType::Short => self._get_short(),
            ValueType::Int | ValueType::UnsignedInt => self.to_unsigned_int() as u16,
            ValueType::Float | ValueType::Double32 | ValueType::Double64 => self.to_double() as u16,
            _ => 0,
        }
    }

    #[allow(clippy::match_same_arms)]
    #[allow(clippy::cast_possible_wrap)]
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn to_int(&self) -> i64 {
        match self.value_type() {
            ValueType::True => 1,
            ValueType::False => 0,
            ValueType::Short => i64::from(self.to_short()),
            ValueType::Int | ValueType::UnsignedInt => {
                let count = (self.bytes[0] & 0x07) as usize + 1;
                let mut buf = [0u8; 8];
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        self.bytes[1..].as_ptr(),
                        buf.as_mut_ptr(),
                        count,
                    );
                }
                i64::from_le_bytes(buf)
            }
            ValueType::Float | ValueType::Double32 | ValueType::Double64 => self.to_double() as i64,
            _ => 0,
        }
    }

    #[allow(clippy::cast_sign_loss)]
    #[must_use]
    pub fn to_unsigned_int(&self) -> u64 {
        self.to_int() as u64
    }

    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn to_double(&self) -> f64 {
        match self.value_type() {
            ValueType::Float | ValueType::Double32 => {
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&self.bytes[2..6]);
                f64::from(f32::from_le_bytes(buf))
            }
            ValueType::Double64 => {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&self.bytes[2..10]);
                f64::from_le_bytes(buf)
            }
            _ => self.to_int() as f64,
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn to_float(&self) -> f32 {
        self.to_double() as f32
    }

    #[must_use]
    pub fn to_data(&self) -> &[u8] {
        match self.value_type() {
            ValueType::String | ValueType::Data => self._get_data(),
            _ => &[],
        }
    }

    #[must_use]
    pub fn to_str(&self) -> &str {
        match self.value_type() {
            ValueType::String => std::str::from_utf8(self._get_data()).unwrap_or(""),
            _ => "",
        }
    }
}

// Conversion to equivalent types
impl Value {
    #[must_use]
    pub fn as_array(&self) -> Option<&Array> {
        if self.value_type() == ValueType::Array {
            Some(Array::from_value(self))
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_dict(&self) -> Option<&Dict> {
        if self.value_type() == ValueType::Dict {
            Some(Dict::from_value(self))
        } else {
            None
        }
    }
}

// Fetching & Validation
impl Value {
    /// Finds the root Fleece value in the data. Performs basic validation that the data is
    /// correctly sized. To ensure the validity of the Fleece data, one should also call `RawValue::validate()`
    fn _find_root(data: &[u8]) -> Result<&Self> {
        // Data must be at least 2 bytes, and evenly sized
        if data.is_empty() || data.len() % 2 != 0 {
            return Err(DecodeError::InputIncorrectlySized);
        }
        // Root is 2 bytes at the end of the data
        let root = &data[(data.len() - 2)..];
        let root: &Value = unsafe { std::mem::transmute(root) };

        if root.value_type() == ValueType::Pointer {
            return Pointer::from_value(root).deref_checked(false, data.as_ptr());
        } else if data.len() == 2 {
            return Ok(root);
        }
        Err(DecodeError::RootNotPointer)
    }

    pub(super) fn _validate<const IS_ARR_ELEM: bool>(
        &self,
        is_wide: bool,
        data_start: *const u8,
        data_end: *const u8,
    ) -> Result<()> {
        match self.value_type() {
            ValueType::Array | ValueType::Dict => {
                Array::from_value(self).validate(data_start, data_end)
            }
            ValueType::Pointer => {
                let target = Pointer::from_value(self).deref_checked(is_wide, data_start)?;
                target._validate::<false>(is_wide, data_start, self.bytes.as_ptr())
            }
            _ => {
                // We don't need to validate that array elements fit within the data, as
                // Array::validate already does that. This improves benchmark performance by ~15%.
                if IS_ARR_ELEM
                    || self.bytes.as_ptr() as usize + self.required_size() <= data_end as usize
                {
                    Ok(())
                } else {
                    Err(DecodeError::ValueOutOfBounds {
                        value_type: self.value_type(),
                        required_size: self.required_size(),
                        available_size: data_end as usize - self.bytes.as_ptr() as usize,
                    })
                }
            }
        }
    }

    // The number of bytes required to hold this value
    // For Dict and Array, this does not include the size of elements, only the header
    #[allow(clippy::match_same_arms)]
    #[must_use]
    pub fn required_size(&self) -> usize {
        match self.value_type() {
            ValueType::Null
            | ValueType::Undefined
            | ValueType::False
            | ValueType::True
            | ValueType::Short => 2,
            ValueType::UnsignedInt | ValueType::Int => 2 + (self.bytes[0] & 0x07) as usize,
            ValueType::Float | ValueType::Double32 => 6,
            ValueType::Double64 => 10,
            ValueType::String | ValueType::Data => {
                let data = self._get_data();
                if let Some(last) = data.last() {
                    std::ptr::from_ref(last) as usize - self.bytes.as_ptr() as usize + 1
                } else {
                    0
                }
            }
            // TODO: This is not correct for MutableArray / MutableDict
            ValueType::Array | ValueType::Dict => 2,
            // Pointers are 2 or 4 bytes, depending on context
            ValueType::Pointer => 2,
        }
    }

    pub(crate) fn dict_key_cmp(value1: &Self, value2: &Self, is_wide: bool) -> Ordering {
        debug_assert!(matches!(
            value1.value_type(),
            ValueType::String | ValueType::Pointer | ValueType::Short
        ));
        debug_assert!(matches!(
            value2.value_type(),
            ValueType::String | ValueType::Pointer | ValueType::Short
        ));
        let value_type1 = value1.value_type();
        let value_type2 = value2.value_type();
        match (value_type1, value_type2) {
            // Inline strings
            (ValueType::String, ValueType::String) => value1.to_str().cmp(value2.to_str()),
            // Pointers to strings
            (ValueType::Pointer, ValueType::Pointer) => {
                let val1 = unsafe {
                    Pointer::from_value(value1)
                        .deref_unchecked(is_wide)
                        .to_str()
                };
                debug_assert_ne!(val1, "", "value1 is not a pointer to a string!");
                let val2 = unsafe {
                    Pointer::from_value(value2)
                        .deref_unchecked(is_wide)
                        .to_str()
                };
                debug_assert_ne!(val2, "", "value2 is not a pointer to a string!");
                val1.cmp(val2)
            }
            (ValueType::String, ValueType::Pointer) => {
                let val2 = unsafe {
                    Pointer::from_value(value2)
                        .deref_unchecked(is_wide)
                        .to_str()
                };
                debug_assert_ne!(val2, "", "value2 is not a pointer to a string!");
                value1.to_str().cmp(val2)
            }
            (ValueType::Pointer, ValueType::String) => {
                let val1 = unsafe {
                    Pointer::from_value(value1)
                        .deref_unchecked(is_wide)
                        .to_str()
                };
                debug_assert_ne!(val1, "", "value1 is not a pointer to a string!");
                val1.cmp(value2.to_str())
            }
            // SharedKeys
            (ValueType::Short, ValueType::Short) => {
                value1.to_unsigned_short().cmp(&value2.to_unsigned_short())
            }
            // SharedKeys are sorted first in the dict
            (ValueType::Short, _) => Ordering::Less,
            (_, ValueType::Short) => Ordering::Greater,
            _ => unreachable!(),
        }
    }

    /// Converts a pointer to a `Value`, and validates its size
    pub(super) fn _from_raw<'a>(ptr: *const u8, available_size: usize) -> Result<&'a Value> {
        let target: &Value = unsafe {
            let slice = std::slice::from_raw_parts(ptr, available_size);
            std::mem::transmute(slice)
        };
        if target.len() < 2 || target.required_size() > available_size {
            Err(DecodeError::ValueOutOfBounds {
                value_type: target.value_type(),
                required_size: target.required_size(),
                available_size,
            })
        } else {
            Ok(target)
        }
    }

    /// Converts a pointer to a `RawValue` reference.
    /// # Safety
    /// The caller should ensure the target is a valid `RawValue`.
    #[inline]
    pub(super) unsafe fn _from_raw_unchecked<'a>(
        ptr: *const u8,
        available_size: usize,
    ) -> &'a Value {
        let slice = std::slice::from_raw_parts(ptr, available_size);
        std::mem::transmute(slice)
    }

    /// A convenience to offset self by `count` bytes, then transmute the result to a `RawValue`
    /// with [`Value::_from_raw_unchecked`].
    #[inline]
    pub(super) unsafe fn _offset_unchecked(&self, count: isize, width: u8) -> &Value {
        let target_ptr = unsafe { self.bytes.as_ptr().offset(count) };
        Value::_from_raw_unchecked(target_ptr, width as usize)
    }
}

// Underlying data getters
impl Value {
    #[inline]
    fn _get_short(&self) -> u16 {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(&self.bytes[0..2]);
        buf[0] &= 0x0F;
        u16::from_be_bytes(buf)
    }

    fn _get_data(&self) -> &[u8] {
        if self.bytes.len() < 2 {
            return &[];
        }
        let inline_size = self.bytes[0] & 0x0F;
        if inline_size == 0x0F {
            // varint
            let (bytes_read, size) = varint::read(&self.bytes[1..]);
            if bytes_read == 0 {
                return &[];
            }
            #[allow(clippy::cast_possible_truncation)]
            let end = 1 + bytes_read + size as usize;
            &self.bytes[1 + bytes_read..end]
        } else {
            let end = 1 + inline_size as usize;
            &self.bytes[1..end]
        }
    }

    #[must_use]
    pub fn null() -> &'static Value {
        unsafe { std::mem::transmute(&constants::NULL[0..2]) }
    }

    #[must_use]
    pub fn undefined() -> &'static Value {
        unsafe { std::mem::transmute(&constants::UNDEFINED[0..2]) }
    }

    #[must_use]
    pub fn bool(b: bool) -> &'static Value {
        if b {
            unsafe { std::mem::transmute::<&[u8], &Value>(&constants::TRUE[0..2]) }
        } else {
            unsafe { std::mem::transmute::<&[u8], &Value>(&constants::FALSE[0..2]) }
        }
    }
}

// Mutability
impl Value {
    #[must_use]
    pub fn is_mutable(&self) -> bool {
        self.bytes.as_ptr() as usize & 1 != 0
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tuple = f.debug_tuple(&format!("{:?}", self.value_type()));
        if self.bytes.is_empty() {
            tuple.field(&"Empty");
        }
        match self.value_type() {
            ValueType::Null => tuple.field(&"Null"),
            ValueType::Undefined => tuple.field(&"Undefined"),
            ValueType::False => tuple.field(&"False"),
            ValueType::True => tuple.field(&"True"),
            ValueType::Short => tuple.field(&self.to_short()),
            ValueType::UnsignedInt => tuple.field(&self.to_unsigned_int()),
            ValueType::Int => tuple.field(&self.to_int()),
            ValueType::Float | ValueType::Double32 | ValueType::Double64 => {
                tuple.field(&self.to_double())
            }
            ValueType::String => tuple.field(&self.to_str()),
            ValueType::Data => tuple.field(&self.to_data()),
            ValueType::Array => tuple.field(&Array::from_value(self)),
            ValueType::Dict => tuple.field(&Dict::from_value(self)),
            ValueType::Pointer => {
                let pointer = Pointer::from_value(self);
                let narrow_offset = unsafe { pointer.get_offset(false) };
                let wide_offset = if self.len() >= 4 {
                    unsafe { pointer.get_offset(true) }
                } else {
                    0
                };
                tuple.field(&format!(
                    "{{ if narrow: -{narrow_offset}, if wide: -{wide_offset} }}"
                ))
            }
        };
        tuple.finish()
    }
}

impl Debug for ValueType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueType::Null => write!(f, "Null"),
            ValueType::Undefined => write!(f, "Undefined"),
            ValueType::False => write!(f, "False"),
            ValueType::True => write!(f, "True"),
            ValueType::Short => write!(f, "Short"),
            ValueType::Int => write!(f, "Int"),
            ValueType::UnsignedInt => write!(f, "UnsignedInt"),
            ValueType::Float => write!(f, "Float"),
            ValueType::Double32 => write!(f, "Double32"),
            ValueType::Double64 => write!(f, "Double64"),
            ValueType::String => write!(f, "String"),
            ValueType::Data => write!(f, "Data"),
            ValueType::Array => write!(f, "Array"),
            ValueType::Dict => write!(f, "Dict"),
            ValueType::Pointer => write!(f, "Pointer"),
        }
    }
}

// Null, Undefined, Bool are special values. 4 bits tag + 4 bits special value.
// Short is 4 bits tag + 12 bits int. (range -2048, 2047 inclusive)
// Int is between 1 and 8 bytes, + 1 byte header (H, I, I, I, I, I, I, I, I) (2 - 9)
// Header is 4 bits tag + 1 bit signed / unsigned + 3 bits size (actually size - 1)
// Float is 4 bytes + 1 byte header + empty byte (H, 0, F, F, F, F) (6)
// Header is 4 bits tag + 4 bits 0.
// Double is 8 bytes + 1 byte header + empty byte (H, 0, F, F, F, F, F, F, F, F) (10)
// Header is 4 bits tag + 1000.
// Small strings (0 or 1 bytes) are 4 bits tag + 4 bits size + 1 byte string.
// Strings with 2 <= size <= 14 are 4 bits tag + 4 bits size + x bytes string.
// Strings with size >= 15 are 4 bits tag + 1111 (to mark varint) + x bytes varint + x bytes string.
// Binary data is written the same as long strings.
// Pointer is 2 or 4 bytes. 2 bits tag.
