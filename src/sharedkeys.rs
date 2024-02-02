use crate::Value;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::RwLock;
use crate::encoder::Encoder;

pub struct SharedKeys {
    map: HashMap<Rc<str>, u16>,
    reverse_map: HashMap<u16, Rc<str>>,
    // `RwLock` allows multi-read and single-write access
    lock: RwLock<()>,
    len: u16,
}

impl SharedKeys {
    const MAX_KEYS: u16 = 2048;
    const KEY_LENGTH: u16 = 16;

    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            reverse_map: HashMap::new(),
            lock: RwLock::new(()),
            len: 0,
        }
    }

    pub fn from_state_bytes(data: &[u8]) -> Option<Self> {
        let state_value = Value::from_bytes(data)?;
        Self::from_state_value(&state_value)
    }

    pub fn from_state_value(value: &Value) -> Option<Self> {
        let state = value.as_array()?;
        let mut shared_keys = Self::new();
        for val in state {
            debug_assert!(val.is_string());
            let borrowed_key = val.as_string()?;
            shared_keys.insert(borrowed_key);
        }
        Some(shared_keys)
    }

    pub fn get_state_bytes(&self) -> Box<[u8]> {
        let mut encoder = Encoder::new();
        self.write_state(&mut encoder);
        encoder.finish()
    }

    pub fn write_state(&self, encoder: &mut Encoder) -> Option<()> {
        let _read_guard = self.lock.read().unwrap();
        encoder.begin_array(self.len as usize);
        for key in self.map.keys() {
            encoder.write::<_, str>(key)?;
        }
        encoder.end_array();
        Some(())
    }

    pub fn len(&self) -> usize {
        let _read_guard = self.lock.read().unwrap();
        self.len as usize
    }

    /// Fetch the int key corresponding to the given string key, if it exists
    pub fn encode(&self, string_key: &str) -> Option<u16> {
        let _read_guard = self.lock.read().unwrap();
        if let Some(key) = self.map.get(string_key) {
            return Some(*key);
        }
        None
    }

    /// Fetch the int key corresponding to the given string key, create it if it doesn't exist
    pub fn encode_and_insert(&mut self, string_key: &str) -> Option<u16> {
        if let Some(key) = self.encode(string_key) {
            return Some(key);
        }
        self.insert(string_key)
    }

    pub fn decode(&self, int_key: u16) -> Option<&str> {
        let _read_guard = self.lock.read().unwrap();
        if let Some(key) = self.reverse_map.get(&int_key) {
            return Some(key);
        }
        None
    }

    fn can_add(&self, key: &str) -> bool {
        let _read_guard = self.lock.read().unwrap();
        self.len < SharedKeys::MAX_KEYS
            && key.len() <= SharedKeys::KEY_LENGTH as usize
            && SharedKeys::can_encode(key)
    }

    fn can_encode(key: &str) -> bool {
        for c in key.chars() {
            if !c.is_alphanumeric() && c != '_' && c != '-' {
                return false;
            }
        }
        true
    }

    pub fn insert(&mut self, key: &str) -> Option<u16> {
        debug_assert!(self.can_add(key));
        if !self.can_add(key) {
            return None;
        }
        let owned_key = Rc::from(key);
        self.insert_owned_key(&owned_key)
    }

    fn insert_owned_key(&mut self, key: &Rc<str>) -> Option<u16> {
        // Unwrap is safe here, because `write` only errors if the lock is poisoned, which can only
        // happen if a panic occurs while holding the lock. We don't panic while holding the lock
        let _write_guard = self.lock.write().unwrap();
        if self.map.contains_key(key) {
            return None;
        }
        let value = self.len;
        self.map.insert(key.clone(), value)?;
        self.reverse_map.insert(value, key.clone())?;
        self.len += 1;
        Some(value)
    }
}