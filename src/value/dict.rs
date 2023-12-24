use crate::raw::{RawArray, RawArrayIter, RawValue};

use super::Value;

#[repr(transparent)]
pub struct Dict<'a> {
    raw: &'a RawArray,
}

impl<'a> Dict<'a> {
    pub(crate) fn new(raw: &'a RawValue) -> Self {
        Self {
            raw: RawArray::from_value(raw),
        }
    }

    pub fn get(&self, index: usize) -> Option<(Value<'a>, Value<'a>)> {
        let offset = 2 * index;
        let key = Value::from_raw(self.raw.get(offset)?)?;
        let val = Value::from_raw(self.raw.get(offset + 1)?)?;
        Some((key, val))
    }
}

pub struct DictIterator<'a> {
    raw: RawArrayIter<'a>,
}

impl<'a> Iterator for DictIterator<'a> {
    type Item = (Value<'a>, Value<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let key = Value::from_raw(self.raw.next()?)?;
        let val = Value::from_raw(self.raw.next()?)?;
        Some((key, val))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Dict is an array of key-value pairs, so there are half as many iter items as there are elements
        (self.raw.len() / 2, Some(self.raw.len() / 2))
    }
}

impl<'a> IntoIterator for &Dict<'a> {
    type Item = (Value<'a>, Value<'a>);
    type IntoIter = DictIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        DictIterator {
            raw: self.raw.into_iter(),
        }
    }
}
