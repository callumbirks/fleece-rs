use super::{
    pointer::ValuePointer,
    value::{RawValue, ValueType},
};

#[repr(transparent)]
pub(crate) struct RawArray {
    value: RawValue,
}

impl RawArray {
    #[allow(clippy::inline_always)]
    #[allow(clippy::transmute_ptr_to_ptr)]
    #[inline(always)]
    // As this struct is repr(transparent), it has the same memory layout as its only field, a RawValue.
    // So the transmute is safe.
    pub fn from_value(value: &RawValue) -> &Self {
        unsafe { std::mem::transmute(value) }
    }

    pub fn get(&self, index: usize) -> Option<&RawValue> {
        let width = self.width();
        let offset = index * width as usize;

        if index > self.elem_count() {
            return None;
        }

        #[allow(clippy::cast_possible_wrap)]
        let target = unsafe { self.value.offset_unchecked(2 + offset as isize, width) };
        Some(if target.value_type() == ValueType::Pointer {
            unsafe { ValuePointer::from_value(target).deref_unchecked(self.is_wide()) }
        } else {
            target
        })
    }

    pub fn first(&self) -> Option<&RawValue> {
        if self.is_wide() {
            if self.value.len() < 6 {
                return None;
            }
        } else if self.value.len() < 4 {
            return None;
        }
        Some(unsafe { self.first_unchecked() })
    }

    pub unsafe fn first_unchecked(&self) -> &RawValue {
        self.value.offset_unchecked(2, self.width())
    }

    pub fn is_wide(&self) -> bool {
        self.value.bytes[0] & 0x08 != 0
    }

    pub fn width(&self) -> u8 {
        if self.is_wide() {
            4
        } else {
            2
        }
    }

    pub fn elem_count(&self) -> usize {
        let mut buf = [0_u8; 2];
        buf.copy_from_slice(&self.value.bytes[0..2]);
        let res = (u16::from_be_bytes(buf) & 0x07FF) as usize;
        if self.value.value_type() == ValueType::Dict {
            res * 2
        } else {
            res
        }
    }
}

// Validation
impl RawArray {
    // I found a 10 percent performance improvement with inline(never) for this function.
    // I think the function is heavier than the compiler assumes.
    #[inline(never)]
    pub(super) fn validate(&self, data_start: *const u8, data_end: *const u8) -> bool {
        let is_wide = self.is_wide();
        let width: u8 = if is_wide { 4 } else { 2 };
        let elem_count = self.elem_count();

        let first = unsafe { self.value.bytes.as_ptr().add(2) };
        if (first as usize) + (elem_count * width as usize) > (data_end as usize) {
            return false;
        }

        let mut current = first;

        for _ in 0..elem_count {
            let next = unsafe { current.add(width as usize) };
            if let Some(current_value) = RawValue::from_raw(current, width as usize) {
                if !current_value.validate::<true>(is_wide, data_start, next) {
                    return false;
                }
            } else {
                return false;
            }

            current = next;
        }

        true
    }
}

// Iterator
pub(crate) struct RawArrayIter<'a> {
    current: &'a RawValue,
    width: u8,
    index: usize,
    len: usize,
}

impl RawArrayIter<'_> {
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<'a> Iterator for RawArrayIter<'a> {
    type Item = &'a RawValue;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }

        // `deref_unchecked` is safe here, as the data has already been validated in `RawArray::validate`, and
        // we do bounds checking above.
        let val = if self.current.value_type() == ValueType::Pointer {
            unsafe { ValuePointer::from_value(self.current).deref_unchecked(self.width == 4) }
        } else {
            self.current
        };

        self.current = unsafe {
            self.current
                .offset_unchecked(self.width as isize, self.width)
        };
        self.index += 1;

        Some(val)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a> IntoIterator for &'a RawArray {
    type Item = &'a RawValue;
    type IntoIter = RawArrayIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        RawArrayIter {
            current: unsafe { self.first_unchecked() },
            width: self.width(),
            index: 0,
            len: self.elem_count(),
        }
    }
}
