use std::io::Write;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU16, Ordering};

use dashmap::DashMap;

use crate::encoder::Encoder;
use crate::Value;
use crate::value::ValueType;

pub struct SharedKeys {
    map: Pin<Box<DashMap<Box<str>, u16>>>,
    reverse_map: DashMap<u16, NonNull<Box<str>>>,
    // `RwLock` allows multi-read and single-write access
    len: AtomicU16,
}

impl SharedKeys {
    // 2048 means the max int will be 2047, which fits in a Fleece Short (12 bits).
    const MAX_KEYS: u16 = 2048;
    const MAX_KEY_LENGTH: u16 = 16;

    pub fn new() -> Self {
        let map = Box::pin(DashMap::default());
        Self {
            map,
            reverse_map: DashMap::default(),
            len: AtomicU16::new(0),
        }
    }

    pub fn from_state_bytes(data: &[u8]) -> Option<Self> {
        let state_value = Value::from_bytes(data).ok()?;
        Self::from_state_value(state_value)
    }

    pub fn from_state_value(value: &Value) -> Option<Self> {
        let state = value.as_array()?;
        let mut shared_keys = Self::new();
        for val in state {
            debug_assert!(val.value_type() == ValueType::String);
            let borrowed_key = val.to_str();
            shared_keys.encode_and_insert(borrowed_key)?;
        }
        Some(shared_keys)
    }

    pub fn get_state_bytes(&self) -> Box<[u8]> {
        let mut encoder = Encoder::new();
        self.write_state(&mut encoder);
        let mut vec = encoder.finish();
        // Shrink to fit hopefully avoids `into_boxed_slice` allocating a new buffer
        vec.shrink_to_fit();
        vec.into_boxed_slice()
    }

    pub fn write_state(&self, encoder: &mut Encoder<impl Write>) -> Option<()> {
        encoder.begin_array(self.len.load(Ordering::SeqCst) as usize);
        for entry in self.map.iter() {
            encoder.write_value::<_, str>(entry.key()).ok()?;
        }
        encoder.end_array().ok()
    }

    pub fn len(&self) -> u16 {
        self.len.load(Ordering::SeqCst)
    }

    /// Fetch the int key corresponding to the given string key, if it exists
    pub fn encode(&self, string_key: &str) -> Option<u16> {
        if let Some(entry) = self.map.get(string_key) {
            return Some(*entry.value());
        }
        None
    }

    /// Fetch the int key corresponding to the given string key, create it if it doesn't exist
    pub fn encode_and_insert(&mut self, key: &str) -> Option<u16> {
        if !self.can_add(key) {
            return None;
        }
        self._insert_owned_key(key)
    }

    pub fn decode(&self, int_key: u16) -> Option<&str> {
        self.reverse_map
            .get(&int_key)
            .map(|s| unsafe { s.value().as_ref().as_ref() })
    }

    pub fn can_add(&self, key: &str) -> bool {
        self.len() < SharedKeys::MAX_KEYS
            && key.len() <= SharedKeys::MAX_KEY_LENGTH as usize
            && SharedKeys::can_encode(key)
    }

    pub fn can_encode(key: &str) -> bool {
        for c in key.chars() {
            if !c.is_alphanumeric() && c != '_' && c != '-' {
                return false;
            }
        }
        true
    }

    fn _insert_owned_key(&mut self, key: &str) -> Option<u16> {
        // Unwrap is safe here, because `write` only errors if the lock is poisoned, which can only
        // happen if a panic occurs while holding the lock. We don't panic while holding the lock
        if self.map.contains_key(key) {
            return None;
        }
        let value = self.len();
        let boxed_key = key.to_string().into_boxed_str();
        self.map.insert(boxed_key, value);
        let entry_ref = self.map.get(key).unwrap();
        // self.map is inside a PinBox, so this is safe
        self.reverse_map
            .insert(value, NonNull::from(entry_ref.key()));
        self.len.fetch_add(1, Ordering::SeqCst);
        Some(value)
    }
}

impl Default for SharedKeys {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SharedKeys {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
            reverse_map: self.reverse_map.clone(),
            len: AtomicU16::new(self.len()),
        }
    }
}

unsafe impl Send for SharedKeys {}
unsafe impl Sync for SharedKeys {}
