use crate::encoder::error::EncodeError;
use crate::value::SizedValue;

#[derive(Default)]
pub struct CollectionStack {
    collections: Vec<Collection>,
}

pub enum Collection {
    Array(Array),
    Dict(Dict),
}

pub struct Array {
    pub values: Vec<SizedValue>,
}

pub enum DictKey {
    Inline(SizedValue),
    // SharedKeys
    Shared(u16),
    // We keep an allocated copy of the key for sorting comparison, because the key is already written to the buffer
    Pointer(Box<str>, u32),
}

pub struct DictElement {
    pub key: DictKey,
    pub val: SizedValue,
}

// Dict uses a BTreeMap because they are naturally sorted
pub struct Dict {
    pub values: Vec<DictElement>,
    pub next_key: Option<DictKey>,
}

impl CollectionStack {
    // CollectionStack always starts with a Dict
    pub fn new() -> Self {
        Self::default()
    }

    pub fn top(&self) -> Option<&Collection> {
        self.collections.last()
    }

    pub fn top_mut(&mut self) -> Option<&mut Collection> {
        self.collections.last_mut()
    }

    pub fn empty(&self) -> bool {
        self.collections.is_empty()
    }

    pub fn push_array(&mut self, capacity: usize) -> crate::encoder::Result<()> {
        if let Some(Collection::Dict(dict)) = self.top() {
            // If the current collection is a dict it should have a key to correspond to this array
            if dict.next_key.is_none() {
                return Err(EncodeError::DictWaitingForKey);
            }
        }
        self.collections
            .push(Collection::Array(Array::with_capacity(capacity)));
        Ok(())
    }

    pub fn push_dict(&mut self) -> crate::encoder::Result<()> {
        if let Some(Collection::Dict(dict)) = self.top() {
            // If the current collection is a dict it should have a key to correspond to this dict
            if dict.next_key.is_none() {
                return Err(EncodeError::DictWaitingForKey);
            }
        }
        self.collections.push(Collection::Dict(Dict::new()));
        Ok(())
    }

    pub fn pop(&mut self) -> Option<Collection> {
        if let Some(Collection::Dict(dict)) = self.top() {
            // Can't pop a dict if it has a key waiting for a value
            if dict.next_key.is_some() {
                return None;
            }
        }
        self.collections.pop()
    }
}

impl Array {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, value: SizedValue) {
        self.values.push(value);
    }
}

impl Dict {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            next_key: None,
        }
    }

    pub fn push_key(&mut self, key: DictKey) -> Option<()> {
        if self.next_key.is_some() {
            return None;
        }
        self.next_key = Some(key);
        Some(())
    }

    pub fn push_value(&mut self, value: SizedValue) -> Option<()> {
        self.values.push(DictElement {
            key: self.next_key.take()?,
            val: value,
        });
        Some(())
    }
}
