use super::error::{DecodeError, Result};
use crate::unlikely;
use crate::value::pointer::Pointer;
use crate::value::{varint, Value, ValueType};

#[repr(transparent)]
pub struct Array {
    value: Value,
}

pub const VARINT_COUNT: u16 = 0x07FF;

impl Array {
    #[allow(clippy::inline_always)]
    #[allow(clippy::transmute_ptr_to_ptr)]
    #[inline(always)]
    /// Transmutes a [`Value`] to an [`Array`].
    /// # Safety
    /// You should validate the array created with this function, otherwise it cannot be
    /// considered valid.
    pub(super) fn from_value(value: &Value) -> &Self {
        unsafe { std::mem::transmute(value) }
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        if index > self.len() {
            return None;
        }

        Some(unsafe { self.get_unchecked(index) })
    }

    /// Get and dereference the value at the given index without bounds checking.
    pub(super) unsafe fn get_unchecked(&self, index: usize) -> &Value {
        let width = self.width();
        let offset = index * width as usize;
        #[allow(clippy::cast_possible_wrap)]
        let first_pos = self.first_pos();
        log::trace!("Array/Dict first pos: {first_pos}");
        #[allow(clippy::cast_possible_wrap)]
        let target = self.value._offset_unchecked((first_pos + offset) as isize, width);
        if target.value_type() == ValueType::Pointer {
            Pointer::from_value(target).deref_unchecked(self.is_wide())
        } else {
            target
        }
    }

    pub(super) fn first_pos(&self) -> usize {
        if self.value.len() < 2 {
            return 0;
        }
        let size = self.value._get_short() & VARINT_COUNT;

        if unlikely(size == VARINT_COUNT) {
            let (read, _) = varint::read(&self.value.bytes[2..]);
            // First pos is 2 + varint len
            if read % 2 != 0 {
                // + 1 again if varint len is odd, because all values are 2-byte aligned.
                2 + read + 1
            } else {
                2 + read
            }
        } else {
            2
        }
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

    /// The number of values in this array.
    pub fn len(&self) -> usize {
        let size = self.value._get_short() & VARINT_COUNT;
        if unlikely(size == VARINT_COUNT) {
            let (read, size) = varint::read(&self.value.bytes[2..]);
            #[allow(clippy::cast_possible_truncation)]
            if read == 0 {
                0
            } else if self.value.value_type() == ValueType::Dict {
                size as usize * 2
            } else {
                size as usize
            }
        } else if self.value.value_type() == ValueType::Dict {
            size as usize * 2
        } else {
            size as usize
        }
    }

    /// The first value in the array. Does *NOT* dereference pointers, because the iterator will
    /// need to offset from this value.
    fn _iter_first(&self) -> Option<&Value> {
        if self.len() == 0 {
            return None;
        }

        #[allow(clippy::cast_possible_wrap)]
        Some(unsafe { self.value._offset_unchecked(self.first_pos() as isize, self.width()) })
    }

}

// Validation
impl Array {
    // I found a 10 percent performance improvement on `benches::decode_people` with inline(never)
    // for this function. I think the function is heavier than the compiler assumes.
    #[inline(never)]
    pub(super) fn validate(&self, data_start: *const u8, data_end: *const u8) -> Result<()> {
        let is_wide = self.is_wide();
        let width: usize = if is_wide { 4 } else { 2 };
        let elem_count = self.len();

        let first = unsafe { self.value.bytes.as_ptr().add(self.first_pos()) };
        if unlikely((first as usize) + (elem_count * width) > (data_end as usize)) {
            let available_size = data_end as usize - first as usize;
            return Err(DecodeError::ArrayOutOfBounds {
                count: elem_count,
                width,
                available_size,
                bytes: Box::from(&self.value.bytes[0..available_size])
            });
        }

        let mut current = first;

        for _ in 0..elem_count {
            let next = unsafe { current.add(width) };
            Value::_from_raw(current, width)?._validate::<true>(is_wide, data_start, next)?;
            current = next;
        }

        Ok(())
    }
}

// Iterator
pub struct Iter<'a> {
    next: Option<&'a Value>,
    width: u8,
    index: usize,
    len: usize,
}

impl Iter<'_> {
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next.is_none() || self.index >= self.len {
            return None;
        }

        let current = self.next.unwrap();
        // `deref_unchecked` is safe here, as the data has already been validated in `RawArray::validate`, and
        // we do bounds checking above.
        let current_resolved = if current.value_type() == ValueType::Pointer {
            unsafe { Pointer::from_value(current).deref_unchecked(self.width == 4) }
        } else {
            current
        };

        self.next = Some(unsafe { current._offset_unchecked(self.width as isize, self.width) });
        self.index += 1;

        Some(current_resolved)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len - self.index, Some(self.len - self.index))
    }
}

impl<'a> IntoIterator for &'a Array {
    type Item = &'a Value;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            next: self._iter_first(),
            width: self.width(),
            index: 0,
            len: self.len(),
        }
    }
}
