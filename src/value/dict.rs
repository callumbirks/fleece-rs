use crate::raw::{RawValue, ValueType};

use super::Value;

pub struct Dict<'a> {
    first: &'a RawValue,
}

impl<'a> Dict<'a> {
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

    pub fn get(&self, index: usize) -> Option<(Value<'a>, Value<'a>)> {
        if index >= self.first.arr_len() {
            return None;
        }
        let offset = 2 * index as isize * self.width() as isize + self.width() as isize;
        let key = Value::from_raw(unsafe {
            let val = self.first.offset_unchecked(offset, self.width());
            if val.value_type() == ValueType::Pointer {
                val.deref_unchecked(self.width())
            } else {
                val
            }
        })?;
        let val = Value::from_raw(unsafe {
            let val = self.first.offset_unchecked(offset + self.width() as isize, self.width());
            if val.value_type() == ValueType::Pointer {
                val.deref_unchecked(self.width())
            } else {
                val
            }
        })?;
        Some((key, val))
    }
}

pub struct DictIterator<'a> {
    current: &'a RawValue,
    index: usize,
    pub width: usize,
    // The number of key-value pairs in the dictionary
    pub len: usize,
}

impl<'a> Iterator for DictIterator<'a> {
    type Item = (Value<'a>, Value<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index + 1 > self.len {
            return None;
        }
        let key = Value::from_raw(unsafe {
            let val = self.current.offset_unchecked(self.width as isize, self.width);
            if val.value_type() == ValueType::Pointer {
                val.deref_unchecked(self.width)
            } else {
                val
            }
        })?;
        let val = Value::from_raw(unsafe {
            let val = self.current.offset_unchecked(2 * self.width as isize, self.width);
            self.current = val;
            if val.value_type() == ValueType::Pointer {
                val.deref_unchecked(self.width)
            } else {
                val
            }
        })?;
        self.index += 1;
        Some((key, val))
    }
}

impl<'a> IntoIterator for &Dict<'a> {
    type Item = (Value<'a>, Value<'a>);
    type IntoIter = DictIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        DictIterator {
            current: self.first,
            index: 0,
            width: self.width(),
            len: self.first.arr_len(),
        }
    }
}
