use super::array;
use super::array::Array;
use crate::encoder::AsBoxedValue;
use crate::value::Value;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::ops::Index;

// A Dict is just an Array, but the elements are alternating key, value
#[repr(transparent)]
pub struct Dict {
    pub(crate) array: Array,
}

impl Dict {
    /// Transmutes a [`Value`] to a [`Dict`].
    /// # Safety
    /// You should validate the dict created with this function, otherwise it cannot be
    /// considered valid.
    #[allow(clippy::transmute_ptr_to_ptr)]
    #[inline]
    pub(crate) fn from_value(value: &Value) -> &Self {
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
        let key: Box<Value> = key.as_boxed_value().ok()?;

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
            let cmp = Value::dict_key_cmp(&key, elem.0, self.is_wide());

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
                return Some(elem.1);
            }

            size = right - left;
        }
        None
    }

    /// The first key-value pair in the dict
    #[must_use]
    pub fn first(&self) -> Option<(&Value, &Value)> {
        if self.is_empty() {
            return None;
        }
        Some(unsafe { self._get_unchecked(0) })
    }

    #[must_use]
    pub fn is_wide(&self) -> bool {
        self.array.is_wide()
    }

    #[must_use]
    pub fn width(&self) -> u8 {
        self.array.width()
    }

    /// The number of key-value pairs in this dict.
    #[must_use]
    pub fn len(&self) -> usize {
        self.array.len() / 2
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.array.is_empty()
    }

    pub fn clone_box(&self) -> Box<Dict> {
        unsafe {
            Box::from_raw(Box::into_raw(self.array.value.clone_box()) as *mut Dict)
        }
    }

}

impl Dict {
    unsafe fn _get_unchecked(&self, index: usize) -> (&Value, &Value) {
        let offset = 2 * index;
        let key = self.array.get_unchecked(offset);
        let val = self.array.get_unchecked(offset + 1);
        (key, val)
    }

    fn is_parent_key(value: &Value) -> bool {
        const PARENT_KEY: [u8; 2] = [(crate::value::tag::SHORT << 4) | 0x08, 0];
        value.bytes[..2] == PARENT_KEY
    }

    #[must_use]
    pub fn iter(&self) -> Iter {
        Iter {
            array_iter: self.array.into_iter(),
        }
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
pub struct Iter<'a> {
    pub(crate) array_iter: array::Iter<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a str, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        let key = self.array_iter.next()?.to_str();
        let val = self.array_iter.next()?;
        Some((key, val))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (array_size_hint, _) = self.array_iter.size_hint();
        (array_size_hint / 2, Some(array_size_hint / 2))
    }
}

impl<'a> IntoIterator for &'a Dict {
    type Item = (&'a str, &'a Value);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Debug for Dict {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut out = "Dict {".to_string();
        for (i, (key, value)) in self.into_iter().enumerate() {
            out.push_str(&format!("'{}': Value {{ type: {:?} }}", key, value.value_type()));
            if i < self.len() - 1 {
                out.push(',');
            }
        }
        out.push('}');
        f.write_str(&out)
    }
}
