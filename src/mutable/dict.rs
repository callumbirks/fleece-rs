use alloc::{
    collections::{btree_map, BTreeMap},
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
    /// Returns `None` if the scope does not have a [`Scope::root`], or the root is not a [`Dict`].
    pub fn from_scope(scope: &Scope) -> Option<Self> {
        scope
            .root()
            .and_then(AllocedValue::to_dict)
            .map(|source| Self::copy_with_shared_keys(&source, scope.shared_keys().cloned()))
    }

    fn copy_with_shared_keys(source: &Dict, shared_keys: Option<Arc<SharedKeys>>) -> Self {
        let mut this = Self {
            shared_keys,
            ..Default::default()
        };
        let is_wide = source.is_wide();
        for (k, v) in source {
            let slot = ValueSlot::new_from_fleece(v, is_wide);
            let key = this.encode_key(k);
            this.map.insert(key, slot);
        }
        this
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[must_use]
    pub fn get<'r>(&'r self, key: &str) -> Option<&'r Value> {
        let encoded_key = self.encode_key(key);
        self.map.get(&encoded_key).and_then(ValueSlot::value)
    }

    #[must_use]
    pub fn get_array<'r>(&'r self, key: &str) -> Option<&'r MutableArray> {
        let encoded_key = self.encode_key(key);
        self.map.get(&encoded_key).and_then(ValueSlot::array)
    }

    #[must_use]
    pub fn get_dict<'r>(&'r self, key: &str) -> Option<&'r MutableDict> {
        let encoded_key = self.encode_key(key);
        self.map.get(&encoded_key).and_then(ValueSlot::dict)
    }

    #[must_use]
    pub fn get_array_mut<'r>(&'r mut self, key: &str) -> Option<&'r mut MutableArray> {
        let encoded_key = self.encode_key(key);
        self.map
            .get_mut(&encoded_key)
            .and_then(ValueSlot::array_mut)
    }

    #[must_use]
    pub fn get_dict_mut<'r>(&'r mut self, key: &str) -> Option<&'r mut MutableDict> {
        let encoded_key = self.encode_key(key);
        self.map.get_mut(&encoded_key).and_then(ValueSlot::dict_mut)
    }

    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        let encoded_key = self.encode_key(key);
        self.map.contains_key(&encoded_key)
    }

    /// Set a key in the dictionary to the given value. This inserts if it doesn't exist, or updates if it does.
    /// Accepts any [`Encodable`] value, and encodes it to Fleece.
    pub fn insert<T>(&mut self, key: &str, value: T)
    where
        T: Encodable,
    {
        let encoded_key = self.encode_key(key);
        let slot = ValueSlot::new(value);
        self.map.insert(encoded_key, slot);
    }

    /// Set a key in the dictionary to the given dict. This inserts if it doesn't exist, or updates if it does.
    pub fn insert_dict(&mut self, key: &str, dict: impl Into<MutableDict>) {
        let encoded_key = self.encode_key(key);
        self.map
            .insert(encoded_key.clone(), ValueSlot::new_dict(dict.into()));
    }

    /// Set a key in the dictionary to the given array. This inserts if it doesn't exist, or updates if it does.
    pub fn insert_array(&mut self, key: &str, array: impl Into<MutableArray>) {
        let encoded_key = self.encode_key(key);
        self.map
            .insert(encoded_key.clone(), ValueSlot::new_array(array.into()));
    }

    pub fn remove(&mut self, key: &str) {
        let encoded_key = self.encode_key(key);
        self.map.remove(&encoded_key);
    }

    #[must_use]
    pub fn iter(&self) -> Iter {
        self.into_iter()
    }

    /// Attempts to encode `key` with `SharedKeys`, and return a `Key::Shared`.
    /// Otherwise returns a `Key::String`.
    fn encode_key(&self, key: &str) -> Key {
        self.shared_keys
            .as_ref()
            .and_then(|sk| sk.encode(key))
            .map_or_else(|| Key::String(key.to_string()), Key::Shared)
    }
}

impl Index<&str> for MutableDict {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).unwrap()
    }
}

pub struct Iter<'a> {
    map_iter: btree_map::Iter<'a, Key, ValueSlot>,
    shared_keys: Option<Arc<SharedKeys>>,
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

impl Iter<'_> {
    fn decode_key<'k>(&self, key: &'k Key) -> Option<&'k str> {
        match key {
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
            .and_then(|(key, slot)| Some((self.decode_key(key)?, Ref::new(slot))))
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
            let slot = v.clone();
            new.map.insert(k.clone(), slot);
        }
        new
    }
}

impl From<AllocedDict> for MutableDict {
    fn from(source: AllocedDict) -> Self {
        let shared_keys = Scope::find_shared_keys(source.value.cast::<u8>());
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
        Self::from_scope(scope).ok_or(())
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
