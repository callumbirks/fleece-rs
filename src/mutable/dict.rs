use alloc::{
    collections::{btree_map, BTreeMap, BTreeSet},
    string::{String, ToString},
    sync::Arc,
};
use core::{cmp, ops::Index};

use crate::{
    alloced::{AllocedDict, AllocedValue},
    encoder::Encodable,
    Dict, Scope, SharedKeys, Value, ValueType,
};

use super::{MutableArray, ValueSlot};

#[derive(Debug)]
pub struct MutableDict {
    map: BTreeMap<Key, ValueSlot>,
    allocated_values: BTreeSet<AllocedValue>,
    shared_keys: Option<Arc<SharedKeys>>,
}

impl MutableDict {
    /// Create a new, empty, mutable dict.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn clone_from(source: &Dict) -> Self {
        let shared_keys = Scope::find_shared_keys(source.array.value.bytes.as_ptr());
        Self::copy_with_shared_keys(source, shared_keys)
    }

    /// Create a new, empty, mutable dict which uses the given shared keys to encode keys.
    #[inline]
    #[must_use]
    pub fn new_with_shared_keys(shared_keys: Arc<SharedKeys>) -> Self {
        Self {
            shared_keys: Some(shared_keys),
            ..Default::default()
        }
    }

    /// Create a new mutable dict which copies the [`AllocedDict`] and [`SharedKeys`] from a [`Scope`].
    ///
    /// # Errors
    /// Returns `Err(())` if the scope does not have a [`Scope::root`], or the root is not a [`Dict`].
    pub fn from_scope(scope: &Scope) -> Result<Self, ()> {
        if let Some(source) = scope.root().and_then(AllocedValue::to_dict) {
            Ok(Self::copy_with_shared_keys(
                &source,
                scope.shared_keys().cloned(),
            ))
        } else {
            Err(())
        }
    }

    fn copy_with_shared_keys(source: &Dict, shared_keys: Option<Arc<SharedKeys>>) -> Self {
        let mut this = Self {
            shared_keys,
            ..Default::default()
        };
        for (k, v) in source {
            let slot = super::encode_fleece(&mut this.allocated_values, v, source.is_wide());
            let key = this.encode_key(k);
            this.map.insert(key, slot);
        }
        this
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[must_use]
    pub fn get<'r>(&'r self, key: &str) -> Option<Ref<'r>> {
        let encoded_key = self.encode_key(key);
        self.map.get(&encoded_key).map(Ref::new)
    }

    #[must_use]
    pub fn get_mut<'r>(&'r mut self, key: &str) -> Option<RefMut<'r>> {
        if !self.contains_key(key) {
            return None;
        }
        let encoded_key = self.encode_key(key);

        if self.map.contains_key(&encoded_key) {
            Some(RefMut {
                dict: self,
                key: encoded_key,
            })
        } else {
            None
        }
    }

    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        let encoded_key = self.encode_key(key);
        self.map.contains_key(&encoded_key)
    }

    /// Set a key in the dictionary to the given value. This inserts if it doesn't exist, or updates if it does.
    /// Accepts any [`Encodable`] value, and encodes it to Fleece.
    ///
    /// Returns a reference to the inserted [`Value`].
    pub fn set<T>(&mut self, key: &str, value: T) -> &Value
    where
        T: Encodable,
    {
        let encoded_key = self.encode_key(key);
        let value = super::encode(&mut self.allocated_values, value);
        let slot = self.map.entry(encoded_key).or_insert(ValueSlot::Empty);
        *slot = value;
        slot.value().unwrap()
    }

    pub fn remove(&mut self, key: &str) {
        let encoded_key = self.encode_key(key);
        if !self.map.contains_key(&encoded_key) {
            return;
        }
        self.map.remove(&encoded_key);
    }

    fn encode_key(&self, key: &str) -> Key {
        self.shared_keys
            .as_ref()
            .and_then(|sk| sk.encode(key))
            .map(Key::Shared)
            .unwrap_or_else(|| Key::String(key.to_string()))
    }
}

pub struct Iter<'a> {
    map_iter: btree_map::Iter<'a, Key, ValueSlot>,
    shared_keys: Option<Arc<SharedKeys>>,
}

impl Iter<'_> {
    fn decode_k<'k>(&self, k: &'k Key) -> Option<&'k str> {
        match k {
            Key::Shared(shared) => unsafe {
                self.shared_keys
                    .as_ref()
                    .and_then(|sk| Some(&*core::ptr::from_ref::<str>(sk.decode(*shared)?)))
            },
            Key::String(ref str) => Some(str),
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a str, Ref<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        self.map_iter
            .next()
            .and_then(|(key, slot)| Some((self.decode_k(key)?, Ref { slot })))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.map_iter.size_hint()
    }
}

impl<'a> IntoIterator for &'a MutableDict {
    type Item = (&'a str, Ref<'a>);

    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let map_iter = self.map.iter();
        Iter {
            map_iter,
            shared_keys: self.shared_keys.clone(),
        }
    }
}

impl Default for MutableDict {
    #[inline]
    fn default() -> Self {
        Self {
            map: BTreeMap::default(),
            allocated_values: BTreeSet::default(),
            shared_keys: None,
        }
    }
}

impl Clone for MutableDict {
    fn clone(&self) -> Self {
        let mut new = Self {
            shared_keys: self.shared_keys.clone(),
            ..Default::default()
        };
        for (k, v) in &self.map {
            let slot = match v {
                ValueSlot::Empty => ValueSlot::Empty,
                ValueSlot::Pointer(_) | ValueSlot::Inline(_) => {
                    super::encode_fleece(&mut new.allocated_values, v.value().unwrap(), false)
                }
                ValueSlot::MutableArray(arr) => ValueSlot::MutableArray(arr.clone()),
                ValueSlot::MutableDict(dict) => ValueSlot::MutableDict(dict.clone()),
            };
            new.map.insert(k.clone(), slot);
        }
        new
    }
}

pub struct Ref<'a> {
    slot: &'a ValueSlot,
}

pub struct RefMut<'a> {
    dict: &'a mut MutableDict,
    key: Key,
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
    pub fn value(&self) -> Option<&Value> {
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
        *self.slot_mut() = super::encode(&mut self.dict.allocated_values, value)
    }

    pub fn set_fleece(&mut self, value: &Value) {
        // We can't safely dereference a 'dangling' pointer as only the parent of the pointer knows its width,
        // so don't allow setting a value from a pointer.
        if matches!(value.value_type(), ValueType::Pointer) {
            return;
        }
        *self.slot_mut() = super::encode_fleece(&mut self.dict.allocated_values, value, false);
    }

    pub fn remove(mut self) {
        *self.slot_mut() = ValueSlot::Empty;
    }

    fn slot(&self) -> &ValueSlot {
        &self.dict.map[&self.key]
    }

    fn slot_mut(&mut self) -> &mut ValueSlot {
        self.dict.map.get_mut(&self.key).unwrap()
    }
}

impl From<AllocedDict> for MutableDict {
    fn from(source: AllocedDict) -> Self {
        let shared_keys = Scope::find_shared_keys(source.value as *const u8);
        Self::copy_with_shared_keys(&source, shared_keys)
    }
}

impl TryFrom<&Scope> for MutableDict {
    type Error = ();

    /// Create a new mutable dict which copies the [`AllocedDict`] and [`SharedKeys`] from a [`Scope`].
    ///
    /// # Errors
    /// Returns `Err(())` if the scope does not have a [`Scope::root`], or the root is not a [`Dict`].
    #[inline]
    fn try_from(scope: &Scope) -> Result<Self, Self::Error> {
        Self::from_scope(scope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Shared(u16),
    String(String),
}

impl PartialEq<Value> for Key {
    fn eq(&self, other: &Value) -> bool {
        match (self, other.value_type()) {
            (Key::Shared(shared), ValueType::Short) => shared.eq(&other.to_unsigned_short()),
            (Key::String(key), ValueType::String) => key.eq(other.to_str()),
            _ => false,
        }
    }
}

impl PartialEq<Key> for Value {
    fn eq(&self, other: &Key) -> bool {
        other.eq(self)
    }
}

impl PartialOrd<Value> for Key {
    fn partial_cmp(&self, other: &Value) -> Option<cmp::Ordering> {
        match (self, other.value_type()) {
            (Key::Shared(shared), crate::ValueType::Short) => {
                Some(shared.cmp(&other.to_unsigned_short()))
            }
            (Key::Shared(_), _) => Some(cmp::Ordering::Less),
            (Key::String(_), crate::ValueType::Short) => Some(cmp::Ordering::Greater),
            (Key::String(key), crate::ValueType::String) => Some(key.as_str().cmp(other.to_str())),
            _ => unreachable!(),
        }
    }
}
