use super::value::{RawValue, ValueType};

/// Internally identical to `RawValue`, this is just used to separate out some functionality.
#[repr(transparent)]
pub(super) struct ValuePointer {
    value: RawValue,
}

impl ValuePointer {
    #[allow(clippy::inline_always)]
    #[allow(clippy::transmute_ptr_to_ptr)]
    #[inline(always)]
    pub fn from_value(value: &RawValue) -> &Self {
        unsafe { std::mem::transmute(value) }
    }

    pub(super) fn deref(&self, wide: bool, data_start: *const u8) -> Option<&RawValue> {
        if wide {
            if self.value.bytes.len() < 4 {
                return None;
            }
        } else if self.value.bytes.len() < 2 {
            return None;
        }

        let offset = unsafe {
            if wide {
                self.get_offset::<true>()
            } else {
                self.get_offset::<false>()
            }
        };
        if offset < 2 {
            return None;
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
            return None;
        }

        let target = unsafe { RawValue::from_raw_unchecked(target_ptr, offset) };

        if target.value_type() == ValueType::Pointer {
            return ValuePointer::from_value(target).deref(true, data_start);
        }
        Some(target)
    }

    // This should only be called when the data has already been validated
    pub(super) unsafe fn deref_unchecked(&self, wide: bool) -> &RawValue {
        let offset = if wide {
            self.get_offset::<true>()
        } else {
            self.get_offset::<false>()
        };
        debug_assert_ne!(offset, 0);

        #[allow(clippy::cast_possible_wrap)]
        let target_ptr = self.offset(-(offset as isize));

        let target = RawValue::from_raw_unchecked(target_ptr, offset);

        if target.value_type() == ValueType::Pointer {
            return ValuePointer::from_value(target).deref_unchecked(true);
        }
        target
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    unsafe fn offset(&self, offset: isize) -> *const u8 {
        self.value.bytes.as_ptr().offset(offset)
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    unsafe fn get_offset<const WIDE: bool>(&self) -> usize {
        if WIDE {
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
