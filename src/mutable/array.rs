use core::ops::Index;

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::{
    alloced::{AllocedArray, AllocedValue},
    encoder::Encodable,
    Array, Scope, Value,
};

use super::{MutableDict, ValueSlot};

#[derive(Debug, Default)]
pub struct MutableArray {
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
    /// Returns `None` if the scope does not have a [`Scope::root`], or the root is not an [`Array`].
    pub fn from_scope(scope: &Scope) -> Option<Self> {
        scope
            .root()
            .and_then(AllocedValue::to_array)
            .map(|source| Self::clone_from(&source))
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.list.get(index).and_then(ValueSlot::value)
    }

    pub fn get_array(&self, index: usize) -> Option<&MutableArray> {
        self.list.get(index).and_then(ValueSlot::array)
    }

    pub fn get_dict(&self, index: usize) -> Option<&MutableDict> {
        self.list.get(index).and_then(ValueSlot::dict)
    }

    pub fn get_array_mut(&mut self, index: usize) -> Option<&mut MutableArray> {
        self.list.get_mut(index).and_then(ValueSlot::array_mut)
    }

    pub fn get_dict_mut(&mut self, index: usize) -> Option<&mut MutableDict> {
        self.list.get_mut(index).and_then(ValueSlot::dict_mut)
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

    /// # Panics
    /// Panics if `index >= len`
    pub fn set<T>(&mut self, index: usize, value: T)
    where
        T: Encodable,
    {
        let slot = super::encode(&mut self.allocated_values, value);
        self.replace(index, slot);
    }

    pub fn set_array(&mut self, index: usize, array: MutableArray) {
        self.replace(index, ValueSlot::new_array(array));
    }

    pub fn set_dict(&mut self, index: usize, dict: MutableDict) {
        self.replace(index, ValueSlot::new_dict(dict));
    }

    pub fn set_fleece(&mut self, index: usize, value: &Value) {
        let slot = super::encode_fleece(&mut self.allocated_values, value, false);
        self.replace(index, slot);
    }

    pub fn push<T>(&mut self, value: &T)
    where
        T: Encodable + ?Sized,
    {
        let slot = super::encode(&mut self.allocated_values, value);
        self.list.push(slot);
    }

    pub fn push_fleece(&mut self, value: &Value) {
        let slot = super::encode_fleece(&mut self.allocated_values, value, false);
        self.list.push(slot);
    }

    pub fn remove(&mut self, index: usize) {
        if index >= self.list.len() {
            return;
        }
        // Remove elem at `index` from the new list.
        let slot = self.list.remove(index);
        self.drop_if_allocated(slot);
    }

    #[must_use]
    pub fn iter(&self) -> Iter {
        self.into_iter()
    }

    #[inline]
    fn replace(&mut self, index: usize, slot: ValueSlot) {
        let prev = core::mem::replace(&mut self.list[index], slot);
        self.drop_if_allocated(prev);
    }

    /// If the given [`ValueSlot`] is a [`ValueSlot::Pointer`], remove its allocated backing from
    /// `allocated_values`.
    /// Consumes the slot because otherwise we could be leaving a dangling pointer around.
    fn drop_if_allocated(&mut self, slot: ValueSlot) {
        if let Some(pointer) = slot.pointer() {
            self.allocated_values.remove(&pointer);
        }
        core::mem::drop(slot);
    }
}

impl Index<usize> for MutableArray {
    type Output = Value;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}

pub struct Ref<'a> {
    slot: &'a ValueSlot,
}

impl<'a> Ref<'a> {
    fn new(slot: &'a ValueSlot) -> Self {
        Self { slot }
    }

    #[must_use]
    pub fn is_value(&self) -> bool {
        self.slot.is_inline() || self.slot.is_pointer()
    }

    #[must_use]
    pub fn is_array(&self) -> bool {
        self.slot.is_array()
    }

    #[must_use]
    pub fn is_dict(&self) -> bool {
        self.slot.is_dict()
    }

    #[must_use]
    pub fn value(&self) -> Option<&Value> {
        self.slot.value()
    }

    #[must_use]
    pub fn array(&self) -> Option<&MutableArray> {
        self.slot.array()
    }

    #[must_use]
    pub fn dict(&self) -> Option<&MutableDict> {
        self.slot.dict()
    }
}

pub struct Iter<'a> {
    arr: &'a MutableArray,
    index: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = Ref<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let i = self.index;
        self.index += 1;
        self.arr.list.get(i).map(Ref::new)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.arr.len();
        (len, Some(len))
    }
}

impl<'a> IntoIterator for &'a MutableArray {
    type Item = Ref<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            arr: self,
            index: 0,
        }
    }
}

impl Clone for MutableArray {
    fn clone(&self) -> Self {
        let mut new = Self::default();
        for v in &self.list {
            let slot = match v {
                ValueSlot::Pointer(_) | ValueSlot::Inline(_) => {
                    super::encode_fleece(&mut new.allocated_values, v.value().unwrap(), false)
                }
                ValueSlot::MutableArray(arr) => ValueSlot::MutableArray(arr.clone()),
                ValueSlot::MutableDict(dict) => ValueSlot::MutableDict(dict.clone()),
            };
            new.list.push(slot);
        }
        new
    }
}

impl From<AllocedArray> for MutableArray {
    fn from(source: AllocedArray) -> Self {
        Self::clone_from(&source)
    }
}

impl From<&Array> for MutableArray {
    fn from(value: &Array) -> Self {
        Self::clone_from(value)
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
