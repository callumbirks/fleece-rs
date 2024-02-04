use crate::encoder::Encodable;
use crate::raw::value::{RawValue, ValueType};
use std::io::Write;

pub enum SizedValue {
    Narrow(NarrowValue),
    Wide(WideValue),
}

impl Encodable for SizedValue {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        match self {
            SizedValue::Narrow(narrow) => writer.write_all(&narrow.bytes).ok(),
            SizedValue::Wide(wide) => writer.write_all(&wide.bytes).ok(),
        }
    }

    fn fleece_size(&self) -> usize {
        match self {
            SizedValue::Narrow(_) => 2,
            SizedValue::Wide(_) => 4,
        }
    }
}

pub struct NarrowValue {
    bytes: [u8; 2],
}

pub struct WideValue {
    bytes: [u8; 4],
}

impl NarrowValue {
    pub fn new(tag: u8, tiny: u8, short: u8) -> Self {
        Self {
            bytes: [tag | tiny, short],
        }
    }

    pub fn value_type(&self) -> ValueType {
        ValueType::from_byte(self.bytes[0])
    }

    pub fn as_value(&self) -> &RawValue {
        unsafe { std::mem::transmute(&self.bytes as &[u8]) }
    }

    pub fn as_bytes(&self) -> &[u8; 2] {
        &self.bytes
    }

    pub fn widen(self) -> WideValue {
        WideValue {
            bytes: [self.bytes[0], self.bytes[1], 0, 0],
        }
    }
}

impl WideValue {
    fn new(tag: u8, tiny: u8, short: u8) -> Self {
        Self {
            bytes: [tag | tiny, short, 0, 0],
        }
    }

    fn value_type(&self) -> ValueType {
        ValueType::from_byte(self.bytes[0])
    }

    pub fn as_value(&self) -> &RawValue {
        unsafe { std::mem::transmute(&self.bytes as &[u8]) }
    }

    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.bytes
    }
}
