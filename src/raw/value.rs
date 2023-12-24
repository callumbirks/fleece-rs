use super::{pointer::ValuePointer, RawArray};
use std::fmt::{Display, Formatter};

#[derive(PartialEq, Eq)]
pub enum ValueType {
    Null,
    Undefined,
    False,
    True,
    Short,
    Int,
    UnsignedShort,
    UnsignedInt,
    Float,
    Double,
    String,
    Data,
    Array,
    Dict,
    Pointer,
}

const VARINT_MAX_LEN: usize = 10;

#[repr(u8)]
pub enum Tag {
    Special = 0x30,
    Short = 0x00,
    Int = 0x10,
    Float = 0x20,
    String = 0x40,
    Data = 0x50,
    Array = 0x60,
    Dict = 0x70,
    Pointer = 0x80,
}

#[repr(u8)]
pub enum Special {
    Null = 0x00,
    Undefined = 0x0C,
    False = 0x04,
    True = 0x08,
}

impl ValueType {
    #[inline(always)]
    pub fn from_byte(byte: u8) -> Self {
        match byte & 0xF0 {
            // Some types have extra info in the lower 4 bits
            x if x == Tag::Special as u8 => match byte & 0x0F {
                y if y == Special::Null as u8 => ValueType::Null,
                y if y == Special::Undefined as u8 => ValueType::Undefined,
                y if y == Special::False as u8 => ValueType::False,
                y if y == Special::True as u8 => ValueType::True,
                _ => ValueType::Null,
            },
            // 0x08 is the sign bit
            x if x == Tag::Short as u8 => match byte & 0x08 {
                0x00 => ValueType::Short,
                _ => ValueType::UnsignedShort,
            },
            x if x == Tag::Int as u8 => match byte & 0x08 {
                0x00 => ValueType::Int,
                _ => ValueType::UnsignedInt,
            },
            // For floats, the sign bit signifies the type is double
            x if x == Tag::Float as u8 => match byte & 0x08 {
                0x00 => ValueType::Float,
                _ => ValueType::Double,
            },
            x if x == Tag::String as u8 => ValueType::String,
            x if x == Tag::Data as u8 => ValueType::Data,
            x if x == Tag::Array as u8 => ValueType::Array,
            x if x == Tag::Dict as u8 => ValueType::Dict,
            // Pointers are 0x80 to 0xF0, so we don't compare directly to Tag::Pointer
            _ => ValueType::Pointer,
        }
    }
}

pub mod constants {
    use super::*;

    pub const TRUE: [u8; 2] = [Tag::Special as u8 | Special::True as u8, 0x00];
    pub const FALSE: [u8; 2] = [Tag::Special as u8 | Special::False as u8, 0x00];
    pub const NULL: [u8; 2] = [Tag::Special as u8 | Special::Null as u8, 0x00];
    pub const UNDEFINED: [u8; 2] = [Tag::Special as u8 | Special::Undefined as u8, 0x00];
}

#[repr(transparent)]
pub struct RawValue {
    pub(super) bytes: [u8],
}

// API
impl RawValue {
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Find and validate Fleece data in the given data. It will return a reference to the root
    /// value. The root value will usually be a Dict.
    pub fn from_bytes(data: &[u8]) -> Option<&Self> {
        let root = Self::find_root(data)?;
        let data_end = unsafe { data.as_ptr().add(data.len()) };
        // wide parameter doesn't matter here, as its only used for pointers, and find_root will
        // never return a pointer.
        if root.validate::<false>(false, data.as_ptr(), data_end) {
            Some(root)
        } else {
            None
        }
    }

    /// Like `from_bytes`, but doesn't do any validation, so it should only be used on trusted data.
    /// If you call this on invalid Fleece data, it will probably panic.
    /// The performance uplift of this function is several thousand times.
    pub unsafe fn from_bytes_unchecked(data: &[u8]) -> &Self {
        // Root is 2 bytes at the end of the data
        let root = &data[(data.len() - 2)..];
        let root: &RawValue = std::mem::transmute(root);
        if root.value_type() == ValueType::Pointer {
            return root.as_value_ptr().deref_unchecked(false);
        } else if data.len() == 2 {
            return root;
        }
        panic!("Invalid data");
    }

    // Will cause a panic if bytes is empty
    #[inline(always)]
    pub fn value_type(&self) -> ValueType {
        ValueType::from_byte(self.bytes[0])
    }
}

impl RawValue {}

// Into Conversions
impl RawValue {
    pub fn to_bool(&self) -> bool {
        match self.value_type() {
            ValueType::False => false,
            ValueType::True => true,
            ValueType::Short | ValueType::Int | ValueType::Float | ValueType::Double => {
                self.to_int() != 0
            }
            _ => true,
        }
    }

    pub fn to_int(&self) -> i64 {
        match self.value_type() {
            ValueType::True => 1,
            ValueType::False => 0,
            ValueType::UnsignedShort => self.get_short() as i64,
            ValueType::Short => {
                let i: u16 = self.get_short();
                if i & 0x0800 != 0 {
                    (i as i16 | 0xF000_u16 as i16) as i64
                } else {
                    i as i64
                }
            }
            ValueType::Int | ValueType::UnsignedInt => {
                let count = (self.bytes[0] & 0x07) as usize + 1;
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&self.bytes[1..count]);
                i64::from_le_bytes(buf)
            }
            ValueType::Float | ValueType::Double => self.to_double() as i64,
            _ => 0,
        }
    }

    pub fn to_unsigned_int(&self) -> u64 {
        self.to_int() as u64
    }

    pub fn to_double(&self) -> f64 {
        match self.value_type() {
            ValueType::Float => {
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&self.bytes[2..6]);
                f32::from_le_bytes(buf) as f64
            }
            ValueType::Double => {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&self.bytes[2..10]);
                f64::from_le_bytes(buf)
            }
            _ => self.to_int() as f64,
        }
    }

    pub fn to_float(&self) -> f32 {
        self.to_double() as f32
    }

    pub fn to_data(&self) -> &[u8] {
        match self.value_type() {
            ValueType::String | ValueType::Data => self.get_data(),
            _ => &[],
        }
    }

    pub fn to_str(&self) -> &str {
        match self.value_type() {
            ValueType::String => std::str::from_utf8(self.get_data()).unwrap_or(""),
            _ => "",
        }
    }
}

// Fetching & Validation
impl RawValue {
    /// Finds the root Fleece value in the data. Performs basic validation that the data is
    /// correctly sized. To ensure the validity of the Fleece data, one should also call `RawValue::validate()`
    #[inline(always)]
    fn find_root(data: &[u8]) -> Option<&Self> {
        // Data must be at least 2 bytes, and evenly sized
        if data.is_empty() || data.len() % 2 != 0 {
            return None;
        }
        // Root is 2 bytes at the end of the data
        let root = &data[(data.len() - 2)..];
        let root: &RawValue = unsafe { std::mem::transmute(root) };

        if root.value_type() == ValueType::Pointer {
            return root.as_value_ptr().deref(false, data.as_ptr());
        } else if data.len() == 2 {
            return Some(root);
        }
        None
    }

    pub(super) fn validate<const IS_ARR_ELEM: bool>(
        &self,
        wide: bool,
        data_start: *const u8,
        data_end: *const u8,
    ) -> bool {
        match self.value_type() {
            ValueType::Array | ValueType::Dict => {
                RawArray::from_value(self).validate(data_start, data_end)
            }
            ValueType::Pointer => {
                if let Some(target) = self.as_value_ptr().deref(wide, data_start) {
                    target.validate::<false>(wide, data_start, self.bytes.as_ptr())
                } else {
                    false
                }
            }
            _ => {
                if IS_ARR_ELEM {
                    self.required_size() <= if wide { 4 } else { 2 }
                } else {
                    self.bytes.as_ptr() as usize + self.required_size() <= data_end as usize
                }
            }
        }
    }

    // The number of bytes required to hold this value
    // For Dict and Array, this does not include the size of inline values, only the header
    pub fn required_size(&self) -> usize {
        match self.value_type() {
            ValueType::Null
            | ValueType::Undefined
            | ValueType::False
            | ValueType::True
            | ValueType::UnsignedShort
            | ValueType::Short => 2,
            ValueType::UnsignedInt | ValueType::Int => 2 + (self.bytes[0] & 0x07) as usize,
            ValueType::Float => 6,
            ValueType::Double => 10,
            ValueType::String | ValueType::Data => {
                let data = self.get_data();
                if let Some(last) = data.last() {
                    last as *const u8 as usize - self.bytes.as_ptr() as usize + 1
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

    /// Converts a pointer to a RawValue reference, and validates its size
    pub(super) fn from_raw<'a>(ptr: *const u8, available_size: usize) -> Option<&'a RawValue> {
        let target: &RawValue = unsafe {
            let slice = std::slice::from_raw_parts(ptr, available_size);
            std::mem::transmute(slice)
        };
        if target.len() < 2 {
            return None;
        }

        if target.required_size() > available_size {
            return None;
        }

        Some(target)
    }

    /// Converts a pointer to a RawValue reference.
    /// # Safety
    /// The caller should ensure the target is a valid RawValue.
    #[inline(always)]
    pub(super) unsafe fn from_raw_unchecked<'a>(
        ptr: *const u8,
        available_size: usize,
    ) -> &'a RawValue {
        let slice = std::slice::from_raw_parts(ptr, available_size);
        std::mem::transmute(slice)
    }

    /// A convenience for offset then `from_raw_unchecked`
    #[inline(always)]
    pub(super) unsafe fn offset_unchecked(&self, count: isize, width: u8) -> &RawValue {
        let target_ptr = unsafe { self.bytes.as_ptr().offset(count) };
        RawValue::from_raw_unchecked(target_ptr, width as usize)
    }
}

// Underlying data getters
impl RawValue {
    #[inline(always)]
    fn get_short(&self) -> u16 {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(&self.bytes[0..2]);
        u16::from_be_bytes(buf) & 0x0FFF
    }

    fn get_data(&self) -> &[u8] {
        if self.bytes.is_empty() {
            return &[];
        }
        let size = self.bytes[0] & 0x0F;
        if size == 0x0F {
            // varint
            let (bytes_read, size) = self.get_varint();
            if bytes_read == 0 {
                return &[];
            }
            let end = 1 + bytes_read + size as usize;
            &self.bytes[1 + bytes_read..end]
        } else {
            let end = 1 + size as usize;
            &self.bytes[1..end]
        }
    }

    // Return (bytes_read, size)
    fn get_varint(&self) -> (usize, u64) {
        if self.bytes.len() < 2 {
            return (0, 0);
        }

        if self.bytes.len() == 2 {
            return (1, self.bytes[1] as u64);
        }

        let mut shift = 0;
        let mut res = 0_u64;

        let end: usize = self.bytes.len().min(VARINT_MAX_LEN + 1);

        for (i, byte) in self.bytes[1..end].iter().enumerate() {
            if *byte >= 0x80 {
                res |= ((*byte & 0x7F) as u64) << shift;
                shift += 7;
            } else {
                res |= (*byte as u64) << shift;
                // Make sure the varint is below the max length
                if i == VARINT_MAX_LEN && *byte > 1 {
                    return (0, 0);
                }
                return (i + 1, res);
            }
        }

        (0, 0)
    }
}

// Conversion to internal types
// These types have the same memory layout as RawValue, so transmute is safe
impl RawValue {
    #[inline(always)]
    pub(super) fn as_value_ptr(&self) -> &ValuePointer {
        unsafe { std::mem::transmute(self) }
    }

    #[inline(always)]
    pub(super) fn as_array(&self) -> &RawArray {
        unsafe { std::mem::transmute(self) }
    }
}

// Mutability
impl RawValue {
    pub fn is_mutable(&self) -> bool {
        self.bytes.as_ptr() as usize & 1 != 0
    }
}

impl Display for RawValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.bytes.is_empty() {
            return write!(f, "Empty");
        }
        match self.value_type() {
            ValueType::Null => write!(f, "Null"),
            ValueType::Undefined => write!(f, "Undefined"),
            ValueType::False => write!(f, "False"),
            ValueType::True => write!(f, "True"),
            ValueType::UnsignedShort | ValueType::UnsignedInt => self.to_unsigned_int().fmt(f),
            ValueType::Short | ValueType::Int => self.to_int().fmt(f),
            ValueType::Float | ValueType::Double => self.to_float().fmt(f),
            ValueType::String => self.to_str().fmt(f),
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
