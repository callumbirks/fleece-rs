use std::collections::HashMap;
use crate::raw::sized::{NarrowValue, SizedValue};
use std::io::Write;
use crate::encoder::Encodable;
use crate::raw::value::tag;

pub struct CollectionStack<'a> {
    collections: Vec<Collection<'a>>,
}

impl<'a> CollectionStack<'a> {
    // CollectionStack always starts with a Dict
    pub fn new() -> Self {
        Self {
            collections: vec![Collection::Dict(Dict::new())],
        }
    }

    pub fn push_array(&mut self) -> Option<()> {
        self.collections.push(Collection::Array(Array::new()));
        Some(())
    }

    pub fn push_dict(&mut self) -> Option<()> {
        self.collections.push(Collection::Dict(Dict::new()));
        Some(())
    }

    pub fn pop_write<W: Write>(&mut self, writer: &mut W) -> Option<()> {
        let collection = self.collections.pop()?;
        collection.write_fleece_to(writer)
    }
}

pub enum Collection<'a> {
    Array(Array),
    Dict(Dict<'a>),
}

impl<'a> Encodable for Collection<'a> {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        match self {
            Collection::Array(arr) => arr.write_fleece_to(writer),
            Collection::Dict(dict) => dict.write_fleece_to(writer)
        }
    }

    fn fleece_size(&self) -> usize {
        2
    }
}

pub struct Array {
    values: Vec<SizedValue>,
}

pub struct Dict<'a> {
    values: HashMap<&'a str, SizedValue>,
}

impl Array {
    pub fn new() -> Self {
        Self { values: vec![] }
    }
}

impl Encodable for Array {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        let val = NarrowValue::new(tag::ARRAY, 0, self.values.len() as u8);
        writer.write_all(val.as_bytes()).ok()?;
        for v in &self.values {
            v.write_fleece_to(writer)?;
        }
        Some(())
    }

    // Don't bother to calculate the full size for this function because it isn't used
    fn fleece_size(&self) -> usize {
        2
    }
}

impl<'a> Dict<'a> {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
}

impl<'a> Encodable for Dict<'a> {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        let val = NarrowValue::new(tag::DICT, 0, self.values.len() as u8);
        writer.write_all(val.as_bytes()).ok()?;
        for (k, v) in &self.values {
            k.write_fleece_to(writer)?;
            v.write_fleece_to(writer)?;
        }
        Some(())
    }

    fn fleece_size(&self) -> usize {
        todo!()
    }
}
