use crate::raw::{RawValue, ValueType};

use super::Value;

pub struct Array<'a> {
    first: &'a RawValue,
}

impl<'a> Array<'a> {
    pub fn new(raw: &'a RawValue) -> Self {
        Self { first: raw }
    }

    fn width(&self) -> usize {
        // TODO: sizeof(ValueSlot) for mutable array
        if self.first.arr_is_wide() {
            4
        } else {
            2
        }
    }

    fn get(&self, index: usize) -> Option<Value<'a>> {
        if index >= self.first.arr_len() {
            return None;
        }
        Value::from_raw(unsafe {
            let val = self.first.offset_unchecked(index as isize, self.width());
            if val.value_type() == ValueType::Pointer {
                val.deref_unchecked(self.width())
            } else {
                val
            }
        })
    }
}

pub struct ArrayIterator<'a> {
    current: &'a RawValue,
    index: usize,
    pub width: usize,
    // The number of elements in the array
    pub len: usize,
}

impl<'a> Iterator for ArrayIterator<'a> {
    type Item = Value<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index + 1 > self.len {
            return None;
        }
        self.current = unsafe {
            self.current
                .offset_unchecked(self.width as isize, self.width)
        };
        self.index += 1;
        Value::from_raw(unsafe {
            if self.current.value_type() == ValueType::Pointer {
                self.current.deref_unchecked(self.width)
            } else {
                self.current
            }
        })
    }
}

impl<'a> IntoIterator for &Array<'a> {
    type Item = Value<'a>;
    type IntoIter = ArrayIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ArrayIterator {
            current: self.first,
            index: 0,
            width: self.width(),
            len: self.first.arr_len(),
        }
    }
}
