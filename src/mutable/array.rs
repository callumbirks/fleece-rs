use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::{
    alloced::{AllocedArray, AllocedValue},
    encoder::Encodable,
    Array, Scope, Value, ValueType,
};

use super::{MutableDict, ValueSlot};

#[derive(Debug)]
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
    /// Returns `Err(())` if the scope does not have a [`Scope::root`], or the root is not an [`Array`].
    pub fn from_scope(scope: &Scope) -> Result<Self, ()> {
        if let Some(source) = scope.root().and_then(AllocedValue::to_array) {
            Ok(Self::clone_from(&source))
        } else {
            Err(())
        }
    }

    #[inline]
    pub fn get<'r>(&'r self, index: usize) -> Option<Ref<'r>> {
        self.list.get(index).map(Ref::new)
    }

    #[inline]
    pub fn get_mut<'r>(&'r mut self, index: usize) -> Option<RefMut<'r>> {
        if index < self.list.len() {
            Some(RefMut { array: self, index })
        } else {
            None
        }
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
        // Remove elem at `index` from the new list.
        let slot = self.list.remove(index);
        // If the Value is a pointer, deallocate the Value it points to
        if let Some(pointer) = slot.pointer() {
            self.allocated_values.remove(&pointer);
        }
    }
}

pub struct Ref<'a> {
    slot: &'a ValueSlot,
}

pub struct RefMut<'a> {
    array: &'a mut MutableArray,
    index: usize,
}

impl<'a> Ref<'a> {
    fn new(slot: &'a ValueSlot) -> Self {
        Self { slot }
    }

    pub fn is_value(&self) -> bool {
        self.slot.is_inline() || self.slot.is_pointer()
    }

    pub fn is_array(&self) -> bool {
        self.slot.is_array()
    }

    pub fn is_dict(&self) -> bool {
        self.slot.is_dict()
    }

    pub fn value(&self) -> Option<&Value> {
        self.slot.value()
    }

    pub fn array(&self) -> Option<&MutableArray> {
        self.slot.array()
    }

    pub fn dict(&self) -> Option<&MutableDict> {
        self.slot.dict()
    }
}

impl<'a> RefMut<'a> {
    pub fn value<'b>(&'b self) -> Option<&'b Value>
    where
        'a: 'b,
    {
        self.slot().value()
    }

    pub fn array(&mut self) -> Option<&mut MutableArray> {
        self.slot_mut().array_mut()
    }

    pub fn dict(&mut self) -> Option<&mut MutableDict> {
        self.slot_mut().dict_mut()
    }

    pub fn set<T>(&mut self, value: T)
    where
        T: Encodable,
    {
        *self.slot_mut() = super::encode(&mut self.array.allocated_values, value);
    }

    pub fn set_fleece(&mut self, value: &Value) {
        // We can't safely dereference a 'dangling' pointer as only the parent of the pointer knows its width,
        // so don't allow setting a value from a pointer.
        if matches!(value.value_type(), ValueType::Pointer) {
            return;
        }
        *self.slot_mut() = super::encode_fleece(&mut self.array.allocated_values, value, false);
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
    type Item = Ref<'a>;

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
    type Item = Ref<'a>;
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
            list: Vec::default(),
            allocated_values: BTreeSet::default(),
        }
    }
}

impl Clone for MutableArray {
    fn clone(&self) -> Self {
        let mut new = Self::default();
        for v in &self.list {
            let slot = match v {
                ValueSlot::Empty => ValueSlot::Empty,
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
        Self::clone_from(&value)
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
