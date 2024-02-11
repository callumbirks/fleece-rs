use std::cell::Cell;
use crate::value::sized::SizedValue;
use std::collections::BTreeMap;

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

pub struct DictElement {
    pub key: SizedValue,
    pub val: SizedValue,
}

// Dict uses a BTreeMap because they are naturally sorted
pub struct Dict {
    pub values: Vec<DictElement>,
    pub next_key: Option<SizedValue>,
}

impl CollectionStack {
    // CollectionStack always starts with a Dict
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.collections.len()
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

    pub fn push_array(&mut self, capacity: usize) -> Option<()> {
        if let Some(Collection::Dict(dict)) = self.top() {
            // If the current collection is a dict it should have a key to correspond to this array
            dict.next_key.as_ref()?;
        }
        self.collections
            .push(Collection::Array(Array::with_capacity(capacity)));
        Some(())
    }

    pub fn push_dict(&mut self) -> Option<()> {
        if let Some(Collection::Dict(dict)) = self.top() {
            // If the current collection is a dict it should have a key to correspond to this dict
            dict.next_key.as_ref()?;
        }
        self.collections.push(Collection::Dict(Dict::new()));
        Some(())
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
    pub fn new() -> Self {
        Self { values: vec![] }
    }

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

    pub fn push_key(&mut self, key: SizedValue) -> Option<()> {
        if self.next_key.is_some() {
            return None;
        }
        self.next_key = Some(key);
        Some(())
    }

    pub fn push_value(&mut self, value: SizedValue) -> Option<()> {
        self.values.push(DictElement { key: self.next_key.take()?, val: value });
        Some(())
    }
}
