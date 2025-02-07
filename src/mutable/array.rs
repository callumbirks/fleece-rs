use core::ops::Index;

use alloc::vec::Vec;

use crate::{
    alloced::{AllocedArray, AllocedValue},
    encoder::Encodable,
    Array, Scope, Value,
};

use super::{MutableDict, ValueSlot};

#[derive(Debug, Default, Clone)]
pub struct MutableArray {
    list: Vec<ValueSlot>,
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
        let is_wide = source.is_wide();
        for v in source {
            let slot = ValueSlot::new_from_fleece(v, is_wide);
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
        let slot = ValueSlot::new(value);
        self.replace(index, slot);
    }

    /// Set the entry at `index` to the given array.
    pub fn set_array(&mut self, index: usize, array: impl Into<MutableArray>) {
        self.replace(index, ValueSlot::new_array(array.into()));
    }

    /// Set the entry at `index` to the given array.
    pub fn set_dict(&mut self, index: usize, dict: impl Into<MutableDict>) {
        self.replace(index, ValueSlot::new_dict(dict.into()));
    }

    pub fn set_fleece(&mut self, index: usize, value: &Value) {
        let slot = ValueSlot::new_from_fleece(value, false);
        self.replace(index, slot);
    }

    pub fn push<T>(&mut self, value: &T)
    where
        T: Encodable + ?Sized,
    {
        let slot = ValueSlot::new(value);
        self.list.push(slot);
    }

    pub fn push_fleece(&mut self, value: &Value) {
        let slot = ValueSlot::new_from_fleece(value, false);
        self.list.push(slot);
    }

    pub fn remove(&mut self, index: usize) {
        if index >= self.list.len() {
            return;
        }
        // Remove elem at `index` from the new list.
        self.list.remove(index);
    }

    #[must_use]
    pub fn iter(&self) -> Iter {
        self.into_iter()
    }

    #[inline]
    fn replace(&mut self, index: usize, slot: ValueSlot) {
        let _ = core::mem::replace(&mut self.list[index], slot);
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
        self.slot.is_value()
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
