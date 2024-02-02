use crate::sharedkeys::SharedKeys;
use std::borrow::Borrow;
use std::rc::Rc;

mod encodable;

struct NullValue;
struct UndefinedValue;

// Implementations are in the `encodable` module
pub trait Encodable {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()>;
}

pub struct Encoder {
    bytes: Vec<u8>,
    shared_keys: Rc<SharedKeys>,
}

impl Encoder {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            shared_keys: Rc::new(SharedKeys::new()),
        }
    }

    pub fn write_key(&mut self, key: &str) -> Option<()> {
        key.write_fleece_to(&mut self.bytes)
    }

    /// Write an encodable type to the encoder. The parameter may be any borrowed form of an encodable type.
    pub fn write<R, T>(&mut self, value: &R) -> Option<()>
    where
        R: Borrow<T>,
        T: Encodable + ?Sized,
    {
        value.borrow().write_fleece_to(&mut self.bytes)
    }

    pub fn begin_array(&mut self, count: usize) {
        todo!()
    }

    pub fn end_array(&mut self) {
        todo!()
    }

    pub fn begin_dict(&mut self, count: usize) {
        todo!()
    }

    pub fn end_dict(&mut self) {
        todo!()
    }

    pub fn finish(self) -> Box<[u8]> {
        self.bytes.into_boxed_slice()
    }
}
