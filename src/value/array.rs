use crate::unlikely;
use crate::value::pointer::Pointer;
use crate::value::{Value, ValueType};

#[repr(transparent)]
pub struct Array {
    value: Value,
}

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
        let width = self.width();
        let offset = index * width as usize;

        if index > self.elem_count() {
            return None;
        }

        #[allow(clippy::cast_possible_wrap)]
        let target = unsafe { self.value._offset_unchecked(2 + offset as isize, width) };
        Some(if target.value_type() == ValueType::Pointer {
            unsafe { Pointer::from_value(target).deref_unchecked(self.is_wide()) }
        } else {
            target
        })
    }

    /// Get and dereference the value at the given index without bounds checking.
    pub(super) unsafe fn get_unchecked(&self, index: usize) -> &Value {
        let width = self.width();
        let offset = index * width as usize;
        #[allow(clippy::cast_possible_wrap)]
        let target = self.value._offset_unchecked(2 + offset as isize, width);
        if target.value_type() == ValueType::Pointer {
            Pointer::from_value(target).deref_unchecked(self.is_wide())
        } else {
            target
        }
    }

    pub fn first(&self) -> Option<&Value> {
        if self.elem_count() == 0 {
            return None;
        }
        Some(unsafe { self.first_unchecked() })
    }

    unsafe fn first_unchecked(&self) -> &Value {
        self.value._offset_unchecked(2, self.width())
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
impl Array {
    // I found a 10 percent performance improvement on `benches::decode_people` with inline(never)
    // for this function. I think the function is heavier than the compiler assumes.
    #[inline(never)]
    pub(super) fn validate(&self, data_start: *const u8, data_end: *const u8) -> bool {
        let is_wide = self.is_wide();
        let width: usize = if is_wide { 4 } else { 2 };
        let elem_count = self.elem_count();

        let first = unsafe { self.value.bytes.as_ptr().add(2) };
        if unlikely((first as usize) + (elem_count * width) > (data_end as usize)) {
            return false;
        }

        let mut current = first;

        for _ in 0..elem_count {
            let next = unsafe { current.add(width) };
            if let Some(current_value) = Value::_from_raw(current, width) {
                if unlikely(!current_value._validate::<true>(is_wide, data_start, next)) {
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

        let mut current = self.next.unwrap();
        // `deref_unchecked` is safe here, as the data has already been validated in `RawArray::validate`, and
        // we do bounds checking above.
        current = if current.value_type() == ValueType::Pointer {
            unsafe { Pointer::from_value(current).deref_unchecked(self.width == 4) }
        } else {
            current
        };

        self.next = Some(unsafe { current._offset_unchecked(self.width as isize, self.width) });
        self.index += 1;

        Some(current)
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
            next: self.first(),
            width: self.width(),
            index: 0,
            len: self.elem_count(),
        }
    }
}
