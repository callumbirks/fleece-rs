use alloc::{
    collections::{btree_map, BTreeMap, BTreeSet},
    string::{String, ToString},
    sync::Arc,
};
use core::{cmp, ops::Index, ptr::NonNull};

use crate::{
    alloced::{AllocedDict, AllocedValue},
    encoder::Encodable,
    value::array,
    Dict, Scope, SharedKeys, Value, ValueType,
};

use super::{MutableArray, ValueSlot};

#[derive(Debug)]
pub struct MutableDict {
    source: Option<AllocedDict>,
    shared_keys: Option<Arc<SharedKeys>>,
    map: BTreeMap<Key, ValueSlot>,
    allocated_values: BTreeSet<AllocedValue>,
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
        let mut this = Self::new();
        if let Some(sk) = Scope::find_shared_keys(source.array.value.bytes.as_ptr()) {
            this.shared_keys = Some(sk)
        }
        for (k, v) in source {
            let slot = super::encode_fleece(&mut this.allocated_values, v, source.is_wide());
            let key = this.encode_key(k);
            this.map.insert(key, slot);
        }
        this
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
    #[inline]
    pub fn from_scope(scope: &Scope) -> Result<Self, ()> {
        Self::try_from(scope)
    }

    #[inline]
    pub fn len(&self) -> usize {
        if let Some(source) = &self.source {
            source.len() + self.map.len()
        } else {
            self.map.len()
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        if let Some(source) = &self.source {
            source.is_empty() && self.map.is_empty()
        } else {
            self.map.is_empty()
        }
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        let encoded_key = self.encode_key(key);
        if let Some(val) = self.map.get(&encoded_key) {
            val.value()
        } else if let Some(source) = &self.source {
            source.get(key)
        } else {
            None
        }
    }

    #[must_use]
    pub fn get_mut<'r>(&'r mut self, key: &str) -> Option<RefMut<'r>> {
        if !self.contains_key(key) {
            return None;
        }
        let encoded_key = self.encode_key(key);
        if let Some(_) = self.map.get(&encoded_key) {
            // If we already have a new value for this key, return a reference to it.
            Some(RefMut {
                dict: self,
                key: encoded_key,
            })
        } else if let Some(source) = &self.source {
            // If this key only exists in the source dict, copy and create a ValueSlot for it, and return a reference to it.
            if let Some(v) = source.get(key) {
                let is_wide = self.is_wide();
                let slot = super::encode_fleece(&mut self.allocated_values, v, is_wide);
                self.map.insert(encoded_key.clone(), slot);
                Some(RefMut {
                    dict: self,
                    key: encoded_key,
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
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
        if !self.map.contains_key(&encoded_key)
            && self
                .source
                .as_ref()
                .is_some_and(|source| !source.contains_key(key))
        {
            return;
        }
        self.map.insert(encoded_key, ValueSlot::Empty);
    }

    fn encode_key(&self, key: &str) -> Key {
        self.shared_keys
            .as_ref()
            .and_then(|sk| sk.encode(key))
            .map(Key::Shared)
            .unwrap_or_else(|| Key::String(key.to_string()))
    }

    fn is_wide(&self) -> bool {
        self.source.as_ref().map(|s| s.is_wide()).unwrap_or(false)
    }
}

pub struct Iter<'a> {
    source_iter: Option<array::Iter<'a>>,
    map_iter: btree_map::Iter<'a, Key, ValueSlot>,
    shared_keys: Option<Arc<SharedKeys>>,
    next_source_key: Option<NonNull<Value>>,
    next_map_entry: Option<(NonNull<Key>, NonNull<ValueSlot>)>,
}

impl Iter<'_> {
    fn get_next_source_key(&mut self) -> Option<NonNull<Value>> {
        self.source_iter.as_mut()?.next().map(NonNull::from)
    }

    fn get_next_map_entry(&mut self) -> Option<(NonNull<Key>, NonNull<ValueSlot>)> {
        self.map_iter
            .next()
            .map(|(k, v)| (NonNull::from(k), NonNull::from(v)))
    }

    fn decode_vk<'vk>(&self, vk: &'vk Value) -> Option<&'vk str> {
        match vk.value_type() {
            ValueType::Short => unsafe {
                self.shared_keys.as_ref().and_then(|sk| {
                    Some(&*core::ptr::from_ref::<str>(
                        sk.decode(vk.to_unsigned_short())?,
                    ))
                })
            },
            ValueType::String => Some(vk.to_str()),
            _ => unreachable!(),
        }
    }

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
    type Item = (&'a str, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        let mut source_key = self.next_source_key;
        let mut map_entry = self.next_map_entry;

        unsafe {
            // Skip all empty entries from `map`, and skip entries in `source` which
            // have the same key as empty entries from `map`.
            while map_entry.is_some_and(|(_, v)| v.as_ref().is_empty()) {
                let map_key = map_entry.map(|(k, _)| k.as_ref()).unwrap();
                while source_key
                    .is_some_and(|sk| self.decode_vk(sk.as_ref()).eq(&self.decode_k(map_key)))
                {
                    source_key = self.get_next_source_key();
                }
                map_entry = self.get_next_map_entry();
            }
        }

        if let Some(source_key) = source_key {
            let source_key = unsafe { source_key.as_ref() };
            if let Some((map_key, map_value)) = map_entry {
                let map_key = unsafe { map_key.as_ref() };

                if map_key <= source_key {
                    let key = self.decode_k(map_key)?;
                    self.next_map_entry = self.get_next_map_entry();

                    if map_key == source_key {
                        // Discard current source key and next source value because Map had the same key
                        let _ = self.source_iter.as_mut().and_then(array::Iter::next);
                        self.next_source_key = self.get_next_source_key();
                    }

                    // Unwrap is safe because we previously skipped all empty values
                    Some((key, unsafe { map_value.as_ref().value().unwrap() }))
                } else {
                    let key = self.decode_vk(source_key)?;
                    let value = self.source_iter.as_mut().and_then(array::Iter::next)?;
                    self.next_source_key = self.get_next_source_key();
                    Some((key, value))
                }
            } else {
                let key = self.decode_vk(source_key)?;
                let value = self.source_iter.as_mut().and_then(array::Iter::next)?;
                self.next_source_key = self.get_next_source_key();
                Some((key, value))
            }
        } else if let Some((map_key, map_value)) = map_entry {
            let map_key = unsafe { map_key.as_ref() };
            match map_key {
                Key::Shared(_) => None,
                Key::String(key) => {
                    self.next_map_entry = self.get_next_map_entry();
                    // Unwrap is safe because we previously skipped all empty values
                    Some((key.as_str(), unsafe { map_value.as_ref().value().unwrap() }))
                }
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let source_len = match self.source_iter.as_ref().map(array::Iter::size_hint) {
            Some((len, _)) => len,
            None => 0,
        };
        (source_len, Some(source_len + self.map_iter.len()))
    }
}

impl<'a> IntoIterator for &'a MutableDict {
    type Item = (&'a str, &'a Value);

    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let mut source_iter = self.source.as_ref().map(|s| s.array.iter());
        let next_source_key = source_iter
            .as_mut()
            .and_then(array::Iter::next)
            .map(NonNull::from);
        let mut map_iter = self.map.iter();
        let next_map_entry = map_iter
            .next()
            .map(|(k, v)| (NonNull::from(k), NonNull::from(v)));

        Iter {
            source_iter,
            map_iter,
            shared_keys: self.shared_keys.clone(),
            next_source_key,
            next_map_entry,
        }
    }
}

impl Default for MutableDict {
    #[inline]
    fn default() -> Self {
        Self {
            source: None,
            shared_keys: None,
            map: BTreeMap::default(),
            allocated_values: BTreeSet::default(),
        }
    }
}

impl Clone for MutableDict {
    fn clone(&self) -> Self {
        let mut new = Self {
            source: self.source.clone(),
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

pub struct RefMut<'a> {
    dict: &'a mut MutableDict,
    key: Key,
}

impl<'a> RefMut<'a> {
    pub fn value(&self) -> Option<&Value> {
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
        *self.slot_mut() = super::encode(&mut self.dict.allocated_values, value)
    }

    pub fn set_fleece(&mut self, value: &Value) {
        let is_wide = self.dict.is_wide();
        *self.slot_mut() = super::encode_fleece(&mut self.dict.allocated_values, value, is_wide)
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

impl Index<&str> for MutableDict {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| panic!("Dict contains no key '{}'", index))
    }
}

impl From<AllocedDict> for MutableDict {
    fn from(source: AllocedDict) -> Self {
        let shared_keys = Scope::find_shared_keys(source.value as *const u8);
        Self {
            source: Some(source),
            shared_keys,
            ..Default::default()
        }
    }
}

impl TryFrom<&Scope> for MutableDict {
    type Error = ();

    /// Create a new mutable dict which copies the [`AllocedDict`] and [`SharedKeys`] from a [`Scope`].
    ///
    /// # Errors
    /// Returns `Err(())` if the scope does not have a [`Scope::root`], or the root is not a [`Dict`].
    fn try_from(scope: &Scope) -> Result<Self, Self::Error> {
        if let Some(source) = scope.root().and_then(AllocedValue::to_dict) {
            Ok(Self {
                source: Some(source),
                shared_keys: scope.shared_keys().cloned(),
                ..Default::default()
            })
        } else {
            Err(())
        }
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
