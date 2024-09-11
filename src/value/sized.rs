use super::{pointer, tag, Value, ValueType};

/// A statically sized [`Value`]. This is always 4 bytes.
/// Necessary to construct values on the stack, for use in the Encoder.
#[derive(Clone, Copy)]
pub struct SizedValue {
    bytes: [u8; 4],
}

impl SizedValue {
    /// Construct a [`SizedValue`] from a 2-byte value.
    #[must_use]
    #[inline]
    pub(crate) fn new_narrow(narrow: [u8; 2]) -> Self {
        Self {
            bytes: [narrow[0], narrow[1], 0, 0],
        }
    }

    #[must_use]
    #[inline]
    pub(crate) fn new_pointer(offset: u32) -> Option<Self> {
        if offset > pointer::MAX_WIDE {
            None
        } else {
            let mut bytes = offset.to_be_bytes();
            bytes[0] |= tag::POINTER;
            Some(Self { bytes })
        }
    }

    #[must_use]
    #[inline]
    pub(crate) fn value_type(&self) -> ValueType {
        ValueType::from_byte(self.bytes[0])
    }

    #[inline]
    pub(crate) fn pointer_offset(&self) -> u32 {
        let mut bytes = self.bytes;
        bytes[0] ^= tag::POINTER;
        u32::from_be_bytes(bytes)
    }

    #[inline]
    pub(crate) fn actual_pointer_offset(&self, out_len: usize) -> u32 {
        (out_len - self.pointer_offset() as usize) as u32
    }

    #[inline]
    pub(crate) fn as_value(&self) -> &Value {
        unsafe { core::mem::transmute(&self.bytes as &[u8]) }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8; 4] {
        &self.bytes
    }
}
