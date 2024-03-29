use super::{pointer, tag, Value, ValueType};

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

    pub(crate) fn narrow_pointer(&self) -> Self {
        Self {
            bytes: [tag::POINTER | self.bytes[2], self.bytes[3], 0, 0],
        }
    }

    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.bytes
    }

    /// Only check this on pointers, the result is garbage for other types
    pub fn is_wide(&self) -> bool {
        self.bytes[0] & 0x3f != 0 || self.bytes[1] != 0 || self.bytes[2] & 0xC0 != 0
    }
}
