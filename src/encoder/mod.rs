use crate::sharedkeys::SharedKeys;
use std::borrow::Borrow;
use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

mod encodable;
mod value_stack;

struct NullValue;
struct UndefinedValue;

// Implementations are in the `encodable` module
pub trait Encodable {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()>;
    fn fleece_size(&self) -> usize;
}

enum LastAdded {
    Key,
    Value,
}

pub struct Encoder {
    out: Vec<u8>,
    shared_keys: Option<Rc<RefCell<SharedKeys>>>,
    // value_stack: [sized::NarrowValue],
    // array_stack: ArrayStack,
    value_stack: value_stack::ValueStack,
    last_added: LastAdded,
}

impl Encoder {
    pub fn new() -> Self {
        Self {
            out: Vec::new(),
            shared_keys: None,
            value_stack: value_stack::ValueStack::new(),
            last_added: LastAdded::Value,
        }
    }

    pub fn new_with_shared_keys(shared_keys: Rc<RefCell<SharedKeys>>) -> Self {
        Self {
            out: Vec::new(),
            shared_keys: Some(shared_keys),
            value_stack: value_stack::ValueStack::new(),
            last_added: LastAdded::Value,
        }
    }

    pub fn write_key(&mut self, key: &str) -> Option<()> {
        if matches!(self.last_added, LastAdded::Key) {
            return None;
        }
        let result = if let Some(shared_keys) = &self.shared_keys {
            let int_key = shared_keys.borrow_mut().encode_and_insert(key)?;
            int_key.write_fleece_to(&mut self.out)
        } else {
            key.write_fleece_to(&mut self.out)
        };
        if result.is_some() {
            self.last_added = LastAdded::Key;
        }
        result
    }

    /// Write an [`Encodable`] type to the encoder. The parameter may be any borrowed form of an Encodable type.
    pub fn write<R, T>(&mut self, value: &R) -> Option<()>
    where
        R: Borrow<T> + ?Sized,
        T: Encodable + ?Sized,
    {
        if matches!(self.last_added, LastAdded::Value) {
            return None;
        }
        let value = value.borrow();
        let result: Option<()> = if value.fleece_size() > 4 {
            // Pointer, add value to output and pointer to stack
            todo!()
        } else {
            // Inline, add to stack
            todo!()
        };
        if result.is_some() {
            self.last_added = LastAdded::Value;
        }
        result
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

    pub fn finish(mut self) -> Box<[u8]> {
        // Shrink because `into_boxed_slice` will allocate a new buffer if the capacity is greater than the length
        self.out.shrink_to_fit();
        self.out.into_boxed_slice()
    }

    pub fn set_shared_keys(&mut self, shared_keys: Rc<RefCell<SharedKeys>>) {
        self.shared_keys = Some(shared_keys);
    }
}
