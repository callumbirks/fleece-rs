use super::{pointer, tag, Value, ValueType};
use crate::likely;
use crate::value::pointer::Pointer;
use std::cmp::Ordering;

/// A statically sized [`Value`]. This is always 4 bytes.
#[derive(Clone)]
pub struct SizedValue {
    bytes: [u8; 4],
}

impl SizedValue {
    pub fn from_narrow(narrow: [u8; 2]) -> Self {
        Self {
            bytes: [narrow[0], narrow[1], 0, 0],
        }
    }

    /// Create a new `SizedValue` from a `u32` offset.
    /// # WARNING
    /// This only uses the lower 2 bytes for narrow pointers, so that they can be easily detected and fixed later
    /// by the [`Encoder::_fix_pointer`] method. When you want to create a narrow pointer, use `new_narrow_pointer`
    /// instead.
    pub fn new_pointer(offset: u32) -> Option<Self> {
        // TODO: Is this check necessary?
        if offset > pointer::MAX_WIDE {
            return None;
        }
        if offset <= u32::from(pointer::MAX_NARROW) {
            let mut bytes: [u8; 4] = [tag::POINTER, 0, 0, 0];
            #[allow(clippy::cast_possible_truncation)]
            bytes[2..].copy_from_slice(&(offset as u16 >> 1).to_be_bytes());
            Some(Self { bytes })
        } else {
            let mut bytes: [u8; 4] = (offset >> 1).to_be_bytes();
            bytes[0] |= tag::POINTER;
            Some(Self { bytes })
        }
    }

    pub fn new_narrow_pointer(offset: u16) -> Self {
        let mut bytes = [0_u8; 4];
        bytes[0..2].copy_from_slice(&(offset >> 1).to_be_bytes());
        bytes[0] |= tag::POINTER;
        Self { bytes }
    }

    pub fn value_type(&self) -> ValueType {
        ValueType::from_byte(self.bytes[0])
    }

    pub fn as_value(&self) -> &Value {
        unsafe { std::mem::transmute(&self.bytes as &[u8]) }
    }

    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.bytes
    }

    // Only used for pointer, as Pointer is the only value stored inline which can be wide
    pub fn is_wide(&self) -> bool {
        self.bytes[0] & 0x3f != 0 || self.bytes[1] != 0 || self.bytes[2] & 0xC0 != 0
    }
}
impl PartialEq<Self> for SizedValue {
    fn eq(&self, other: &Self) -> bool {
        match (self.value_type(), other.value_type()) {
            (ValueType::Pointer, ValueType::Pointer) => unsafe {
                let val = Pointer::from_value(self.as_value()).deref_unchecked(self.is_wide());
                let other = Pointer::from_value(other.as_value()).deref_unchecked(other.is_wide());
                val.eq(other)
            },
            (ValueType::Pointer, _) => unsafe {
                let val = Pointer::from_value(self.as_value()).deref_unchecked(self.is_wide());
                val.eq(other.as_value())
            },
            (_, ValueType::Pointer) => unsafe {
                let other = Pointer::from_value(other.as_value()).deref_unchecked(other.is_wide());
                self.as_value().eq(other)
            },
            // Inline values are compared by their bytes
            _ => self.bytes[0] == other.bytes[0] && self.bytes[1] == other.bytes[1],
        }
    }
}

impl Eq for SizedValue {}

impl PartialOrd for SizedValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.as_value().cmp(other.as_value()))
    }
}

/// Just use `Value` implementation.
impl Ord for SizedValue {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_value().cmp(other.as_value())
    }
}
