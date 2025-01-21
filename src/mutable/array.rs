use core::ops::Index;

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::{
    alloced::{AllocedArray, AllocedValue},
    encoder::Encodable,
    Array, Scope, Value,
};

use super::{MutableDict, ValueSlot};

#[derive(Debug)]
pub struct MutableArray {
    source: Option<AllocedArray>,
    list: Vec<ValueSlot>,
    allocated_values: BTreeSet<AllocedValue>,
}

impl MutableArray {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn clone_from(source: &Array) -> Self {
        let mut this = Self::new();
        for v in source {
            let slot = super::encode_fleece(&mut this.allocated_values, v, source.is_wide());
            this.list.push(slot);
        }
        this
    }

    /// Create a new mutable array which copies the [`AllocedArray`] from a [`Scope`].
    ///
    /// # Errors
    /// Returns `Err(())` if the scope does not have a [`Scope::root`], or the root is not an [`Array`].
    #[inline]
    pub fn from_scope(scope: &Scope) -> Result<Self, ()> {
        Self::try_from(scope)
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Value> {
        if index >= self.list.len() {
            return None;
        }
        if let Some(value) = self.list[index].value() {
            Some(value)
        } else if let Some(source) = &self.source {
            Some(&source[index])
        } else {
            None
        }
    }

    pub fn get_mut<'r>(&'r mut self, index: usize) -> Option<RefMut<'r>> {
        if index >= self.list.len() {
            return None;
        }
        if self.list[index].is_empty() {
            let source = self
                .source
                .clone()
                .expect("If a slot is empty, it is delegating to source");
            // If the slot is empty (we are delegating to source), copy the source value into the slot
            self.list[index] =
                super::encode_fleece(&mut self.allocated_values, &source[index], source.is_wide());
        }
        Some(RefMut { array: self, index })
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn set<T>(&mut self, index: usize, value: T) -> Option<&Value>
    where
        T: Encodable,
    {
        if index >= self.list.len() {
            return None;
        }
        let slot = super::encode(&mut self.allocated_values, value);
        let _ = core::mem::replace(&mut self.list[index], slot);
        self.list[index].value()
    }

    pub fn push<T>(&mut self, value: &T) -> &Value
    where
        T: Encodable + ?Sized,
    {
        let slot = super::encode(&mut self.allocated_values, value);
        self.list.push(slot);
        self.list.last().unwrap().value().unwrap()
    }

    pub fn remove(&mut self, index: usize) {
        if index >= self.list.len() {
            return;
        }
        // Copy all of the elements after `index` from the source array to the new list.
        self.copy_source(index + 1);
        // Remove elem at `index` from the new list.
        let slot = self.list.remove(index);
        if slot.is_pointer() {
            // Deallocate the allocated value
            if let Some(pointer) = slot.pointer() {
                self.allocated_values.remove(&pointer);
            }
        }
    }

    fn is_wide(&self) -> bool {
        self.source.as_ref().map(|s| s.is_wide()).unwrap_or(false)
    }

    fn copy_source(&mut self, from_index: usize) {
        let Some(source) = self.source.clone() else {
            return;
        };
        let is_wide = source.is_wide();
        for (i, value) in source.iter().enumerate().skip(from_index) {
            if self.list[i].is_empty() {
                self.list[i] = super::encode_fleece(&mut self.allocated_values, value, is_wide);
            }
        }
    }
}

pub struct RefMut<'a> {
    array: &'a mut MutableArray,
    index: usize,
}

impl<'a> RefMut<'a> {
    pub fn value<'b>(&'b self) -> Option<&'b Value>
    where
        'a: 'b,
    {
        self.slot().value()
    }

    pub fn array(&mut self) -> Option<&mut MutableArray> {
        self.slot_mut().array()
    }

    pub fn dict(&mut self) -> Option<&mut MutableDict> {
        self.slot_mut().dict()
    }

    pub fn set<T>(&mut self, value: T)
    where
        T: Encodable,
    {
        *self.slot_mut() = super::encode(&mut self.array.allocated_values, value);
    }

    pub fn set_fleece(&mut self, value: &Value) {
        let is_wide = self.array.is_wide();
        *self.slot_mut() = super::encode_fleece(&mut self.array.allocated_values, value, is_wide);
    }

    pub fn remove(mut self) {
        *self.slot_mut() = ValueSlot::Empty;
    }

    fn slot(&self) -> &ValueSlot {
        &self.array.list[self.index]
    }

    fn slot_mut(&mut self) -> &mut ValueSlot {
        &mut self.array.list[self.index]
    }
}

pub struct Iter<'a> {
    arr: &'a MutableArray,
    index: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        let i = self.index;
        self.index += 1;
        self.arr.get(i)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.arr.len();
        (len, Some(len))
    }
}

impl<'a> IntoIterator for &'a MutableArray {
    type Item = &'a Value;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            arr: self,
            index: 0,
        }
    }
}

impl Default for MutableArray {
    fn default() -> Self {
        Self {
            source: None,
            list: Vec::default(),
            allocated_values: BTreeSet::default(),
        }
    }
}

impl Clone for MutableArray {
    fn clone(&self) -> Self {
        let mut new = Self {
            source: self.source.clone(),
            ..Default::default()
        };
        let is_wide = self.is_wide();
        for v in &self.list {
            let slot = match v {
                ValueSlot::Empty => ValueSlot::Empty,
                ValueSlot::Pointer(_) | ValueSlot::Inline(_) => {
                    super::encode_fleece(&mut new.allocated_values, v.value().unwrap(), is_wide)
                }
                ValueSlot::MutableArray(arr) => ValueSlot::MutableArray(arr.clone()),
                ValueSlot::MutableDict(dict) => ValueSlot::MutableDict(dict.clone()),
            };
            new.list.push(slot);
        }
        new
    }
}

impl Index<usize> for MutableArray {
    type Output = Value;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| panic!("Index {} out of bounds for MutableArray", index))
    }
}

impl From<AllocedArray> for MutableArray {
    fn from(source: AllocedArray) -> Self {
        let list = core::iter::repeat(ValueSlot::Empty)
            .take(source.len())
            .collect();
        Self {
            source: Some(source),
            list,
            ..Default::default()
        }
    }
}

impl From<&Array> for MutableArray {
    fn from(value: &Array) -> Self {
        let list = core::iter::repeat(ValueSlot::Empty)
            .take(value.len())
            .collect();
        Self {
            list,
            ..Default::default()
        }
    }
}

impl TryFrom<&Scope> for MutableArray {
    type Error = ();

    /// Create a new mutable array which copies the [`AllocedArray`] from a [`Scope`].
    ///
    /// # Errors
    /// Returns `Err(())` if the scope does not have a [`Scope::root`], or the root is not an [`Array`].
    fn try_from(scope: &Scope) -> Result<Self, Self::Error> {
        if let Some(source) = scope.root().and_then(AllocedValue::to_array) {
            Ok(Self::from(source))
        } else {
            Err(())
        }
    }
}
