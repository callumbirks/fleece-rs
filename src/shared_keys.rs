use std::io::Write;

use fixedstr::zstr;

use crate::{Encoder, Value, ValueType};

pub struct SharedKeys(folklore::HashMap<zstr<16>, u16>);

impl SharedKeys {
    const MAX_KEYS: u16 = 2048;
    const MAX_KEY_LEN: u16 = 16;

    #[inline]
    pub fn new() -> Self {
        Self(folklore::HashMap::with_capacity(Self::MAX_KEYS as usize))
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn encode(&self, string_key: &str) -> Option<u16> {
        self.0.get(&zstr::make(string_key))
    }

    // This function takes a `&mut self` because it is not technically thread-safe, another thread
    // could insert a key between `index = self.0.len()` and `self.0.insert()`.
    pub fn encode_and_insert(&mut self, key: &str) -> Option<u16> {
        if !self.can_add(key) {
            return None;
        }
        let key = zstr::make(key);
        if self.0.contains_key(&key) {
            return None;
        }
        let index = self.0.len() as u16;
        if !self.0.insert(key, index) {
            return None;
        }
        Some(index)
    }

    #[inline]
    pub fn decode(&self, int_key: u16) -> Option<&str> {
        self.0.get_key(int_key as usize).map(|s| s.as_str())
    }

    pub fn can_add(&self, key: &str) -> bool {
        (self.len() as u16) < Self::MAX_KEYS
            && key.len() <= Self::MAX_KEY_LEN as usize
            && Self::can_encode(key)
    }

    #[inline]
    fn can_encode(key: &str) -> bool {
        key.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    }

    #[inline]
    pub fn from_state_bytes(data: &[u8]) -> Option<Self> {
        let state_value = Value::from_bytes(data).ok()?;
        Self::from_state_value(state_value)
    }

    pub fn from_state_value(value: &Value) -> Option<Self> {
        let state = value.as_array()?;
        let mut shared_keys = Self::new();
        for val in state {
            debug_assert_eq!(val.value_type(), ValueType::String);
            let borrowed_key = val.to_str();
            shared_keys.encode_and_insert(borrowed_key)?;
        }
        Some(shared_keys)
    }

    pub fn get_state_bytes(&self) -> Box<[u8]> {
        let mut encoder = Encoder::new();
        self.write_state(&mut encoder);
        let mut vec = encoder.finish();
        vec.shrink_to_fit();
        vec.into_boxed_slice()
    }

    pub fn write_state(&self, encoder: &mut Encoder<impl Write>) -> Option<()> {
        if encoder.begin_array(self.0.len()).is_err() {
            return None;
        }
        for (key, _) in &self.0 {
            encoder.write_value::<_, str>(key.as_str()).ok()?;
        }
        encoder.end_array().ok()
    }
}

impl Clone for SharedKeys {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
