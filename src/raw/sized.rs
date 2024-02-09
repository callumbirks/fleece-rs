use crate::raw::pointer;
use crate::raw::value::{tag, RawValue, ValueType};

#[derive(Eq, PartialEq, Hash, Clone)]
pub struct SizedValue {
    pub bytes: [u8; 4],
}

impl SizedValue {
    pub fn from_narrow(narrow: [u8; 2]) -> Self {
        Self {
            bytes: [narrow[0], narrow[1], 0, 0],
        }
    }

    pub fn new_pointer(offset: u32) -> Option<Self> {
        // TODO: Is this check necessary?
        if offset > pointer::MAX_WIDE {
            return None;
        }
        if offset <= pointer::MAX_NARROW as u32 {
            let mut bytes: [u8; 2] = (offset as u16 >> 1).to_be_bytes();
            bytes[0] |= tag::POINTER;
            Some(Self::from_narrow(bytes))
        } else {
            let mut bytes: [u8; 4] = (offset >> 1).to_be_bytes();
            bytes[0] |= tag::POINTER;
            Some(Self { bytes })
        }
    }

    pub fn value_type(&self) -> ValueType {
        ValueType::from_byte(self.bytes[0])
    }

    pub fn as_value(&self) -> &RawValue {
        unsafe { std::mem::transmute(&self.bytes as &[u8]) }
    }

    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.bytes
    }

    // Only used for pointer, as Pointer is the only value stored inline which can be wide
    pub fn is_wide(&self) -> bool {
        self.bytes[2] != 0
    }
}
