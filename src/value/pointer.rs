use super::error::Result;
use super::{Value, ValueType};
use crate::unlikely;
use crate::value::error::DecodeError;

/// Internally identical to `RawValue`, this is just used to separate out some functionality.
#[repr(transparent)]
pub(crate) struct Pointer {
    value: Value,
}

// The maximum offset that can be stored by a Fleece pointer, while being able to fit the tag, and the external tag
pub const MAX_NARROW: u16 = 0x3fff;
pub const MAX_WIDE: u32 = 0x3fff_ffff;

impl Pointer {
    #[allow(clippy::inline_always)]
    #[allow(clippy::transmute_ptr_to_ptr)]
    #[inline(always)]
    pub fn from_value(value: &Value) -> &Self {
        unsafe { std::mem::transmute(value) }
    }

    pub(super) fn deref_checked(&self, wide: bool, data_start: *const u8) -> Result<&Value> {
        if unlikely((wide && self.value.bytes.len() < 4) || self.value.bytes.len() < 2) {
            return Err(DecodeError::PointerTooSmall {
                actual: self.value.bytes.len(),
                expected: if wide { 4 } else { 2 },
            });
        }

        let offset = unsafe { self.get_offset(wide) };
        if unlikely(offset == 0) {
            return Err(DecodeError::PointerOffsetZero);
        }

        // First get the pointer given by offset, so we can validate before de-referencing
        #[allow(clippy::cast_possible_wrap)]
        let target_ptr = unsafe { self.offset(-(offset as isize)) };

        // Is this pointer external to the source data?
        if unlikely(self.value.bytes[0] & 0x40 != 0) {
            // return resolve_external_pointer(target_ptr, data_start, data_end);
            unimplemented!()
            // If the pointer isn't external, it should fit within the source data
        } else if unlikely(target_ptr < data_start) {
            return Err(DecodeError::PointerTargetOutOfBounds {
                offset,
                data_start: data_start as usize,
                target: target_ptr as usize,
            });
        }

        let target = unsafe { Value::_from_raw_unchecked(target_ptr, offset) };

        if unlikely(target.value_type() == ValueType::Pointer) {
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

        let target = Value::_from_raw_unchecked(target_ptr, offset);

        if unlikely(target.value_type() == ValueType::Pointer) {
            return Pointer::from_value(target).deref_unchecked(true);
        }
        target
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    unsafe fn offset(&self, offset: isize) -> *const u8 {
        let narrow_offset = unsafe { self.get_offset(false) };
        let wide_offset = if self.value.len() >= 4 {
            unsafe { self.get_offset(true) }
        } else {
            0
        };
        log::trace!("Dereferencing Pointer {{ if narrow: {narrow_offset}, if wide: {wide_offset} }}, using {offset}");
        self.value.bytes.as_ptr().offset(offset)
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub unsafe fn get_offset(&self, wide: bool) -> usize {
        if wide {
            let mut buf = [0u8; 4];
            buf.copy_from_slice(&self.value.bytes[0..4]);
            ((u32::from_be_bytes(buf) & !0xC000_0000) << 1) as usize
        } else {
            let mut buf = [0u8; 2];
            buf.copy_from_slice(&self.value.bytes[0..2]);
            ((u16::from_be_bytes(buf) & !0xC000) << 1) as usize
        }
    }
}
