use super::array::Array;
use super::{array, ValueType};
use crate::encoder::{AsBoxedValue, Encodable};
use crate::scope::Scope;
use crate::sharedkeys::SharedKeys;
use crate::value::Value;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::ops::Index;

// A Dict is just an Array, but the elements are alternating key, value
#[repr(transparent)]
pub struct Dict {
    array: Array,
}

pub struct Element<'a> {
    pub key: &'a Value,
    pub val: &'a Value,
}

impl Dict {
    /// Transmutes a [`Value`] to a [`Dict`].
    /// # Safety
    /// You should validate the dict created with this function, otherwise it cannot be
    /// considered valid.
    #[allow(clippy::transmute_ptr_to_ptr)]
    #[inline]
    pub(super) fn from_value(value: &Value) -> &Self {
        unsafe { std::mem::transmute(value) }
    }

    pub fn contains_key<R>(&self, key: &R) -> bool
    where
        R: ?Sized + Borrow<str>,
    {
        self.get(key).is_some()
    }

    pub fn get<R>(&self, key: &R) -> Option<&Value>
    where
        R: ?Sized + Borrow<str>,
    {
        let key: &str = key.borrow();
        let key: Box<Value> = self.encode_key(key)?;

        // We use binary search to find the key. This is possible because the dict keys are sorted.
        // This binary search implementation is borrowed from https://doc.rust-lang.org/std/vec/struct.Vec.html#method.binary_search_by

        let mut size = self.len();
        let mut left = 0;
        let mut right = size;
        while left < right {
            let mid = left + size / 2;

            // SAFETY: the while condition means `size` is strictly positive, so
            // `size/2 < size`. Thus `left + size/2 < left + size`, which
            // coupled with the `left + size <= self.len()` invariant means
            // we have `left + size/2 < self.len()`, and this is in-bounds.
            let elem = unsafe { self._get_unchecked(mid) };
            let cmp = Value::dict_key_cmp(&key, elem.key, self.is_wide());

            // This control flow produces conditional moves, which results in
            // fewer branches and instructions than if/else or matching on
            // cmp::Ordering.
            // This is x86 asm for u8: https://rust.godbolt.org/z/698eYffTx.
            left = if cmp == Ordering::Greater {
                mid + 1
            } else {
                left
            };
            right = if cmp == Ordering::Less { mid } else { right };
            if cmp == Ordering::Equal {
                // SAFETY: same as the `get_unchecked` above
                return Some(elem.val);
            }

            size = right - left;
        }
        None
    }

    /// The first key-value pair in the dict
    pub fn first(&self) -> Option<Element> {
        if self.len() == 0 {
            return None;
        }
        Some(unsafe { self._get_unchecked(0) })
    }

    pub fn is_wide(&self) -> bool {
        self.array.is_wide()
    }

    pub fn width(&self) -> u8 {
        self.array.width()
    }

    /// The number of key-value pairs in this dict.
    pub fn len(&self) -> usize {
        self.array.len() / 2
    }
}

impl Dict {
    unsafe fn _get_unchecked(&self, index: usize) -> Element {
        let offset = 2 * index;
        let key = self.array.get_unchecked(offset);
        let val = self.array.get_unchecked(offset + 1);
        Element { key, val }
    }

    /// Attempt to encode a key string to a `Value`. If this `Dict` uses `SharedKeys`, and they can 
    /// be found, and the key exists in the shared keys, the returned value will be a short with the
    /// corresponding encoded key.
    /// Otherwise, the returned `Value` will be a String containing the input key.
    fn encode_key(&self, key: &str) -> Option<Box<Value>> {
        if self.uses_shared_keys() {
            let first = unsafe { self._get_unchecked(0).key };
            if let Some(shared_keys) = Scope::find_shared_keys(first.bytes.as_ptr()) {
                if let Some(encoded) = shared_keys.encode(key) {
                    return encoded.as_boxed_value().ok();
                }
            }
        }
        key.as_boxed_value().ok()
    }

    fn uses_shared_keys(&self) -> bool {
        let len = self.len();
        if len == 0 {
            return false;
        }
        let first_key = unsafe { self._get_unchecked(0).key };

        if Dict::is_parent_key(first_key) {
            if len > 1 {
                let second_key = unsafe { self._get_unchecked(1).key };
                second_key.value_type() == ValueType::Short
            } else {
                false
            }
        } else {
            first_key.value_type() == ValueType::Short
        }
    }

    fn is_parent_key(value: &Value) -> bool {
        const PARENT_KEY: [u8; 2] = [(crate::value::tag::SHORT << 4) | 0x08, 0];
        value.bytes[..2] == PARENT_KEY
    }
}

impl Index<&str> for Dict {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).expect("Key not found")
    }
}

// As a Dict is just an Array but with alternating key-value pairs, we can use ArrayIterator for
// the implementation of DictIterator.
#[repr(transparent)]
pub struct Iter<'a> {
    array_iter: array::Iter<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = Element<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let key = self.array_iter.next()?;
        let val = self.array_iter.next()?;
        Some(Element { key, val })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (array_size_hint, _) = self.array_iter.size_hint();
        (array_size_hint / 2, Some(array_size_hint / 2))
    }
}

impl<'a> IntoIterator for &'a Dict {
    type Item = Element<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            array_iter: self.array.into_iter(),
        }
    }
}
