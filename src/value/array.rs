use crate::raw::{RawArray, RawArrayIter, RawValue};

use super::Value;

#[repr(transparent)]
pub struct Array<'a> {
    raw: &'a RawArray,
}

impl<'a> Array<'a> {
    pub(crate) fn new(raw: &'a RawValue) -> Self {
        Self {
            raw: RawArray::from_value(raw),
        }
    }

    fn get(&self, index: usize) -> Option<Value<'a>> {
        Value::from_raw(unsafe { self.raw.get(index)? })
    }
}

#[repr(transparent)]
pub struct ArrayIterator<'a> {
    raw: RawArrayIter<'a>,
}

impl<'a> Iterator for ArrayIterator<'a> {
    type Item = Value<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        Value::from_raw(unsafe { self.raw.next()? })
    }
}

impl<'a> IntoIterator for &Array<'a> {
    type Item = Value<'a>;
    type IntoIter = ArrayIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ArrayIterator {
            raw: self.raw.into_iter(),
        }
    }
}
