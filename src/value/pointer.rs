use super::error::Result;
use super::{Value, ValueType};
use crate::value::error::DecodeError;
use std::ptr;

/// Internally identical to `RawValue`, this is just used to separate out some functionality.
#[repr(transparent)]
pub(crate) struct Pointer {
    value: Value,
}

// The maximum offset that can be stored by a Fleece pointer, while being able to fit the tag, and the external tag
pub const MAX_NARROW: u16 = 0x3fff;
pub const MAX_WIDE: u32 = 0x3fff_ffff;

impl Pointer {
    #[allow(clippy::transmute_ptr_to_ptr)]
    #[inline]
    pub fn from_value(value: &Value) -> &Self {
        unsafe { std::mem::transmute(value) }
    }

    pub(crate) fn deref_checked(&self, wide: bool, data_start: *const u8) -> Result<&Value> {
        if (wide && self.value.bytes.len() < 4) || self.value.bytes.len() < 2 {
            return Err(DecodeError::PointerTooSmall {
                actual: self.value.bytes.len(),
                expected: if wide { 4 } else { 2 },
            });
        }

        let offset = unsafe { self.get_offset(wide) };
        if offset == 0 {
            return Err(DecodeError::PointerOffsetZero);
        }

        // First get the pointer given by offset, so we can validate before de-referencing
        #[allow(clippy::cast_possible_wrap)]
        let target_ptr = unsafe { self.offset(-(offset as isize)) };

        // Is this pointer external to the source data?
        if self.value.bytes[0] & 0x40 != 0 {
            // return resolve_external_pointer(target_ptr, data_start, data_end);
            unimplemented!()
            // If the pointer isn't external, it should fit within the source data
        } else if target_ptr < data_start {
            return Err(DecodeError::PointerTargetOutOfBounds {
                data_start: data_start as usize,
                target: target_ptr as usize,
                offset,
            });
        }

        let target = unsafe { Value::_from_raw_unchecked(target_ptr, offset as usize) };

        if target.value_type() == ValueType::Pointer {
            return Pointer::from_value(target).deref_checked(true, data_start);
        }
        Ok(target)
    }

    /// Dereferences the pointer, returning the value it points to.
    /// # Safety
    /// The data should be validated before calling this function.
    pub(crate) unsafe fn deref_unchecked(&self, wide: bool) -> &Value {
        let offset = unsafe { self.get_offset(wide) };
        debug_assert_ne!(offset, 0);

        #[allow(clippy::cast_possible_wrap)]
        let target_ptr = self.offset(-(offset as isize));

        let target = Value::_from_raw_unchecked(target_ptr, offset as usize);

        if target.value_type() == ValueType::Pointer {
            return Pointer::from_value(target).deref_unchecked(true);
        }
        target
    }

    #[inline]
    unsafe fn offset(&self, offset: isize) -> *const u8 {
        self.value.bytes.as_ptr().offset(offset)
    }

    #[inline]
    pub unsafe fn get_offset(&self, wide: bool) -> u32 {
        if wide {
            let mut buf = [0u8; 4];
            ptr::copy_nonoverlapping(self.value.bytes.as_ptr(), buf.as_mut_ptr(), 4);
            (u32::from_be_bytes(buf) & !0xC000_0000) * 2
        } else {
            let mut buf = [0u8; 2];
            ptr::copy_nonoverlapping(self.value.bytes.as_ptr(), buf.as_mut_ptr(), 2);
            u32::from((u16::from_be_bytes(buf) & !0xC000) * 2)
        }
    }
}
