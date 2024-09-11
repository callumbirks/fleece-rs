use super::{pointer, tag, Value, ValueType};

/// A statically sized [`Value`]. This is always 4 bytes.
/// Necessary to construct values on the stack, for use in the Encoder.
#[derive(Clone)]
pub(crate) struct SizedValue {
    bytes: [u8; 4],
}

const WIDE_POINTER_TAG: u8 = 0x80;
const NARROW_POINTER_TAG: u8 = 0xC0;

impl SizedValue {
    /// Construct a [`SizedValue`] from a 2-byte value.
    #[must_use]
    pub fn from_narrow(narrow: [u8; 2]) -> Self {
        Self {
            bytes: [narrow[0], narrow[1], 0, 0],
        }
    }

    #[must_use]
    pub fn value_type(&self) -> ValueType {
        ValueType::from_byte(self.bytes[0])
    }

    pub(crate) fn as_value(&self) -> &Value {
        unsafe { core::mem::transmute(&self.bytes as &[u8]) }
    }

    pub(crate) fn as_bytes(&self) -> &[u8; 4] {
        &self.bytes
    }

    /// Create a new `SizedValue` from a `u32` offset.
    /// # WARNING
    /// This only uses the lower 2 bytes for narrow pointers, so that they can be easily detected and fixed later
    /// by the [`Encoder::_fix_pointer`] method. When you want to create a valid Fleece pointer, use
    /// `new_narrow_pointer` / `new_wide_pointer`.
    //pub(crate) fn new_temp_pointer(offset: u32) -> Option<Self> {
    //    // TODO: Is this check necessary?
    //    if offset > pointer::MAX_WIDE {
    //        return None;
    //    }
    //    if offset <= u32::from(pointer::MAX_NARROW) {
    //        let mut bytes: [u8; 4] = [NARROW_POINTER_TAG, 0, 0, 0];
    //        #[allow(clippy::cast_possible_truncation)]
    //        bytes[2..].copy_from_slice(&(offset as u16 >> 1).to_be_bytes());
    //        Some(Self { bytes })
    //    } else {
    //        Self::new_wide_pointer(offset)
    //    }
    //}

    pub(crate) fn new_pointer(offset: u32) -> Option<Self> {
        if offset <= pointer::MAX_NARROW as u32 {
            Self::new_narrow_pointer(offset as u16)
        } else {
            Self::new_wide_pointer(offset)
        }
    }

    pub(crate) fn new_wide_pointer(offset: u32) -> Option<Self> {
        if offset > pointer::MAX_WIDE {
            return None;
        }
        let mut bytes: [u8; 4] = (offset >> 1).to_be_bytes();
        bytes[0] |= WIDE_POINTER_TAG;
        Some(Self { bytes })
    }

    pub(crate) fn new_narrow_pointer(offset: u16) -> Option<Self> {
        if offset > pointer::MAX_NARROW {
            return None;
        }
        let mut bytes = [0_u8; 4];
        bytes[0..2].copy_from_slice(&(offset >> 1).to_be_bytes());
        bytes[0] |= NARROW_POINTER_TAG;
        Some(Self { bytes })
    }

    //pub(crate) fn narrow_pointer(&self) -> Self {
    //    Self {
    //        bytes: [NARROW_POINTER_TAG | self.bytes[2], self.bytes[3], 0, 0],
    //    }
    //}

    /// Only check this on pointers, the result is garbage for other types
    #[must_use]
    pub fn is_wide(&self) -> bool {
        self.bytes[0] & 0xC0 == WIDE_POINTER_TAG
    }

    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn actual_offset(&self, out_len: usize) -> u32 {
        (out_len
            - unsafe {
                pointer::Pointer::from_value(self.as_value()).get_offset(self.is_wide()) as usize
            }) as u32
    }
}
