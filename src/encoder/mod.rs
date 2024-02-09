use crate::encoder::value_stack::{Collection, CollectionStack};
use crate::raw::pointer;
use crate::raw::pointer::ValuePointer;
use crate::raw::sized::SizedValue;
use crate::raw::value::{tag, ValueType};
use crate::sharedkeys::SharedKeys;
use std::borrow::Borrow;
use std::io::{Read, Write};

mod encodable;
mod value_stack;

struct NullValue;
struct UndefinedValue;

// Implementations are in the `encodable` module
pub trait Encodable {
    /// Write self to the given writer, encoded as Fleece. Return [`None`] if the value is too large to be written.
    /// Return [`Some`] with the number of bytes written if the value was written successfully.
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize>;
    fn fleece_size(&self) -> usize;
    fn to_value(&self) -> Option<SizedValue>;
}

enum LastAdded {
    Key,
    Value,
}

pub struct Encoder<W: Write> {
    out: W,
    shared_keys: Option<SharedKeys>,
    collection_stack: CollectionStack,
    len: usize,
}

impl<'a> Encoder<Vec<u8>> {
    pub fn new() -> Encoder<Vec<u8>> {
        Self {
            out: Vec::new(),
            shared_keys: None,
            collection_stack: CollectionStack::new(),
            len: 0,
        }
    }
}

impl<'a, W: Write> Encoder<W> {
    pub fn new_to_writer(out: W) -> Self {
        Self {
            out,
            shared_keys: None,
            collection_stack: CollectionStack::new(),
            len: 0,
        }
    }

    pub fn write_key(&mut self, key: &str) -> Option<()> {
        if let Some(val) = key.to_value() {
            let Collection::Dict(dict) = self.collection_stack.top_mut()? else {
                return None;
            };
            // If the key is small enough to fit in a fixed-width value, inline it
            dict.push_key(val)
        } else if let Some(shared_keys) = &mut self.shared_keys {
            let Collection::Dict(dict) = self.collection_stack.top_mut()? else {
                return None;
            };
            // If we have shared keys, insert the key into the shared keys and add the corresponding int key to the Dict
            let int_key = shared_keys.encode_and_insert(key)?;
            // Unwrap is safe here because `u16::to_value` will only fail if the value is > 2047, which it will never
            // be because the shared_keys holds max 2048 keys (and the first one is 0)
            dict.push_key(int_key.to_value().unwrap())
        } else {
            // If we don't have shared keys, write the key to the output buffer and add a pointer to it in the Dict
            let offset = self._write(key, false)?;
            let Collection::Dict(dict) = self.collection_stack.top_mut()? else {
                return None;
            };
            dict.push_key(SizedValue::new_pointer(offset)?)
        }
    }

    /// Write an [`Encodable`] type to the encoder. The parameter may be any borrowed form of an Encodable type.
    // `R: Borrow<T>` enables us to pass something like an Rc<T> directly to this function
    pub fn write_value<R, T>(&mut self, value: &R) -> Option<()>
    where
        R: Borrow<T> + ?Sized,
        T: Encodable + ?Sized,
    {
        if self.collection_stack.empty() {
            return None;
        }

        let value = value.borrow();
        if let Some(val) = value.to_value() {
            // If the value can fit in a fixed-width Value, just push it to the current collection
            self._push(val)
        } else {
            // Otherwise, write it to output and push a pointer to it onto the current collection
            let offset = self._write(value, false)?;
            let pointer = SizedValue::new_pointer(offset)?;
            self._push(pointer)
        }
    }

    pub fn begin_array(&mut self, capacity: usize) {
        self.collection_stack.push_array(capacity);
    }

    pub fn end_array(&mut self) -> Option<()> {
        let Collection::Array(mut array) = self.collection_stack.pop()? else {
            return None;
        };
        let is_wide = self._array_should_be_wide(&array);
        self._fix_array_pointers(&mut array, is_wide);
        let array_value = SizedValue::from_narrow([tag::ARRAY, array.values.len() as u8]);
        self._write(&array_value, is_wide);

        for v in array.values {
            self._write(&v, is_wide);
        }
        Some(())
    }

    pub fn begin_dict(&mut self, capacity: usize) {
        self.collection_stack.push_dict(capacity);
    }

    pub fn end_dict(&mut self) -> Option<()> {
        match self.collection_stack.top() {
            // Can only end a dict if the top collection is a dict
            Some(Collection::Dict(dict)) => {
                // That dict must not have a key waiting for a value
                if dict.next_key.is_some() {
                    return None;
                }
            }
            _ => return None,
        }
        let Collection::Dict(mut dict) = self.collection_stack.pop()? else {
            unreachable!()
        };
        // TODO: VARINT FOR LARGE DICTS

        let offset_from_start = self.len;

        let is_wide = self._dict_should_be_wide(&dict);

        self._fix_dict_pointers(&mut dict, is_wide);

        let byte0 = if is_wide { tag::DICT | 0x08 } else { tag::DICT };
        let dict_value = SizedValue::from_narrow([byte0, dict.values.len() as u8]);
        self._write(&dict_value, is_wide);

        // TODO: SORT DICT
        for (k, v) in &dict.values {
            self._write_dict_key(k, is_wide);
            self._write(v, is_wide);
        }

        self._finished_collection(offset_from_start as u32);

        Some(())
    }

    pub fn finish(mut self) -> W {
        self._end();
        self.out.flush().ok();
        self.out
    }

    pub fn shared_keys(&self) -> Option<SharedKeys> {
        self.shared_keys.clone()
    }

    /// Consumes the shared keys given, so they can be safely appended to. When the encoder is finished, call
    /// [`Encoder::shared_keys`] to get the updated shared keys.
    pub fn set_shared_keys(&mut self, shared_keys: SharedKeys) {
        self.shared_keys = Some(shared_keys);
    }
}

// private
impl<W: Write> Encoder<W> {
    // Always use this function to write values to the output buffer, because it makes sure all values
    // are evenly aligned.
    /// Write a value to the output buffer and return the offset at which it was written.
    /// The offset can be used to create a pointer to the value.
    fn _write<T: Encodable + ?Sized>(&mut self, value: &T, is_wide: bool) -> Option<u32> {
        let offset = self.len as u32;
        self.len += value.write_fleece_to(&mut self.out, is_wide)?;
        // Pad to even
        if self.len % 2 != 0 {
            self.out.write_all(&[0]).ok()?;
            self.len += 1;
        }
        Some(offset)
    }

    /// Push a fixed-width Fleece value to the collection which is currently at the top of the stack.
    /// This function will return [`None`] if the top of the stack is not a collection, or if it is a
    /// [`Collection::Dict`] and the last addition was not a key.
    fn _push(&mut self, value: SizedValue) -> Option<()> {
        match self.collection_stack.top_mut()? {
            Collection::Array(arr) => {
                arr.push(value);
                Some(())
            }
            Collection::Dict(dict) => dict.push_value(value),
            _ => None,
        }
    }

    fn _end(&mut self) {
        if self.collection_stack.len() == 1 {
            self.end_dict();
        }
    }

    fn _actual_pointer_offset(&self, offset_from_start: u32) -> u32 {
        self.len as u32 - offset_from_start
    }

    fn _array_should_be_wide(&self, array: &value_stack::Array) -> bool {
        for v in &array.values {
            if v.is_wide() {
                return true;
            }
        }
        false
    }

    // Only Pointer might take more than 2 bytes, if any do then the whole dict needs to be wide
    fn _dict_should_be_wide(&self, dict: &value_stack::Dict) -> bool {
        for (k, v) in &dict.values {
            if k.value_type() == ValueType::Pointer {
                let pointer = ValuePointer::from_value(k.as_value());
                let offset = if k.is_wide() {
                    unsafe { pointer.get_offset::<true>() }
                } else {
                    unsafe { pointer.get_offset::<false>() }
                };
                if offset > pointer::MAX_NARROW as usize {
                    return true;
                }
            }
            if v.is_wide() {
                return true;
            }
        }
        false
    }

    fn _fix_pointer(&self, pointer: &SizedValue, len: usize, is_wide: bool) -> SizedValue {
        let pointer = ValuePointer::from_value(pointer.as_value());
        let offset = if is_wide {
            unsafe { pointer.get_offset::<true>() }
        } else {
            unsafe { pointer.get_offset::<false>() }
        };
        let offset = len - offset;
        SizedValue::new_pointer(offset as u32).unwrap()
    }

    fn _fix_array_pointers(&mut self, array: &mut value_stack::Array, is_wide: bool) {
        let mut len = self.len;
        for v in &mut array.values {
            if v.value_type() == ValueType::Pointer {
                let pointer = self._fix_pointer(v, len, is_wide);
                *v = pointer;
            }
            if is_wide {
                len += 4;
            } else {
                len += 2;
            }
        }
    }

    fn _fix_dict_pointers(&mut self, dict: &mut value_stack::Dict, is_wide: bool) {
        let mut len = self.len;
        for (_, v) in &mut dict.values {
            if is_wide {
                len += 8;
            } else {
                len += 4;
            }
            if v.value_type() == ValueType::Pointer {
                let pointer = self._fix_pointer(v, len, is_wide);
                *v = pointer;
            }
        }
    }

    fn _write_dict_key(&mut self, key: &SizedValue, is_wide: bool) {
        match key.value_type() {
            ValueType::String | ValueType::Short => {
                self._write(key, is_wide);
            }
            ValueType::Pointer => {
                let pointer = ValuePointer::from_value(key.as_value());

                let offset = if key.is_wide() {
                    unsafe { pointer.get_offset::<true>() }
                } else {
                    unsafe { pointer.get_offset::<false>() }
                };
                let offset = self._actual_pointer_offset(offset as u32);
                let pointer = SizedValue::new_pointer(offset).unwrap();
                self._write(&pointer, is_wide);
            }
            _ => unreachable!(),
        }
    }

    fn _finished_collection(&mut self, offset_from_start: u32) {
        if let Some(collection) = self.collection_stack.top_mut() {
            match collection {
                Collection::Dict(dict) => {
                    let pointer = SizedValue::new_pointer(offset_from_start).unwrap();
                    dict.push_value(pointer);
                }
                Collection::Array(array) => {
                    let pointer = SizedValue::new_pointer(offset_from_start).unwrap();
                    array.push(pointer);
                }
            }
        } else {
            // The last collection is finished, write the root value at the end.
            // This root value points to the outermost collection.
            let root =
                SizedValue::new_pointer(self._actual_pointer_offset(offset_from_start)).unwrap();
            self._write(&root, false);
        }
    }
}
