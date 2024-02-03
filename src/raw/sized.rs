use crate::raw::value::{RawValue, ValueType};

pub enum SizedValue {
    Narrow(NarrowValue),
    Wide(WideValue),
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
        unsafe { std::mem::transmute(&self.bytes[..]) }
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
        unsafe { std::mem::transmute(&self.bytes[..]) }
    }
}
