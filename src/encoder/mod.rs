use crate::encoder::error::EncodeError;
use crate::encoder::value_stack::{Collection, CollectionStack, DictKey};
use crate::sharedkeys::SharedKeys;
use crate::value;
use crate::value::pointer::Pointer as ValuePointer;
use crate::value::sized::SizedValue;
use crate::value::{pointer, ValueType};
use error::Result;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::io::Write;
use std::sync::Arc;

mod encodable;
mod error;
mod value_stack;

use crate::scope::Scope;
pub use encodable::AsBoxedValue;

struct NullValue;
struct UndefinedValue;

// Implementations are in the `encodable` module
pub trait Encodable {
    /// Write self to the given writer, encoded as Fleece. Return [`None`] if any write operations fail.
    /// Return [`Some`] with the number of bytes written if the value was written successfully.
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> std::io::Result<usize>;
    /// The number of bytes necessary to encode this value in Fleece.
    fn fleece_size(&self) -> usize;
    /// Construct a [`SizedValue`] from this value, if this value can be represented in 2 bytes of Fleece. Otherwise,
    /// return [`None`].
    /// Use [`SizedValue::from_narrow`] to construct the value.
    fn to_sized_value(&self) -> Option<SizedValue>;
}

#[derive(Default)]
pub struct Encoder<W: Write> {
    out: W,
    shared_keys: Option<SharedKeys>,
    collection_stack: CollectionStack,
    len: usize,
}

impl Encoder<Vec<u8>> {
    #[must_use]
    pub fn new() -> Encoder<Vec<u8>> {
        Self::default()
    }
}

impl<W: Write> Encoder<W> {
    pub fn new_to_writer(out: W) -> Self {
        Self {
            out,
            shared_keys: None,
            collection_stack: CollectionStack::new(),
            len: 0,
        }
    }

    /// Write the key string to this `Encoder`.
    /// ## Errors
    /// - If there is not an open Dict, or the top-level open collection is an Array.
    /// - If the last item pushed to the Dict was a key (it is waiting for a value).
    /// - I/O errors related to writing to this Encoder's writer.
    pub fn write_key(&mut self, key: &str) -> Result<()> {
        if let Some(val) = key.to_sized_value() {
            // Keys which are small enough are inlined.
            self._write_key_narrow(val)
        } else {
            self._write_key(key)
        }
    }

    /// Write an [`Encodable`] type to the encoder. The parameter may be any borrowed form of an Encodable type.
    /// `R: Borrow<T>` enables us to pass something like an Rc<T> directly to this function
    /// ## Errors
    /// - If there is not an open collection (Array/Dict).
    /// - If the open collection is a Dict, and it is waiting for a key.
    /// - I/O Errors related to writing to this Encoder's writer.
    pub fn write_value<R, T>(&mut self, value: &R) -> Result<()>
    where
        R: Borrow<T> + ?Sized,
        T: Encodable + ?Sized,
    {
        if self.collection_stack.empty() {
            return Err(EncodeError::CollectionNotOpen);
        }

        let value = value.borrow();
        if let Some(val) = value.to_sized_value() {
            // If the value can fit in a fixed-width Value, just push it to the current collection
            self._push(val)
        } else {
            // Otherwise, write it to output and push a pointer to it onto the current collection
            let offset = self._write(value, false)?;
            let pointer =
                SizedValue::new_pointer(offset).ok_or_else(|| EncodeError::PointerTooLarge)?;
            self._push(pointer)
        }
    }

    /// Write a Fleece `Value` to the Encoder. If the value is an `Array` or `Dict`, all the
    /// elements will be written as well. This function cannot validate Fleece `Array` or `Dict`,
    /// so ensure they are valid before passing them to this function.
    /// ## Errors
    /// - If there is not an open collection (Array/Dict).
    /// - If the open collection is a Dict, and it is waiting for a key.
    /// - If the value is invalid Fleece.
    /// - I/O errors related to writing to this Encoder's writer.
    pub fn write_fleece(&mut self, value: &value::Value) -> Result<()> {
        // If the encoder has no open collections and the value is not a collection, return None
        if self.collection_stack.empty()
            && value.value_type() != ValueType::Dict
            && value.value_type() != ValueType::Array
        {
            return Err(EncodeError::CollectionNotOpen);
        }
        match value.value_type() {
            ValueType::True => self._push(SizedValue::from_narrow(value::constants::TRUE)),
            ValueType::False => self._push(SizedValue::from_narrow(value::constants::FALSE)),
            ValueType::Null => self._push(SizedValue::from_narrow(value::constants::NULL)),
            ValueType::Undefined => {
                self._push(SizedValue::from_narrow(value::constants::UNDEFINED))
            }
            ValueType::Short => self.write_value(&value.to_short()),
            ValueType::UnsignedInt => self.write_value(&value.to_unsigned_int()),
            ValueType::Int => self.write_value(&value.to_int()),
            ValueType::Float => self.write_value(&value.to_float()),
            ValueType::Double32 | ValueType::Double64 => self.write_value(&value.to_double()),
            ValueType::String => self.write_value(value.to_str()),
            ValueType::Data => self.write_value(value.to_data()),
            ValueType::Array => {
                let Some(array) = value.as_array() else {
                    unreachable!()
                };
                self.begin_array(array.len());
                for val in array {
                    self.write_fleece(val)?;
                }
                self.end_array()
            }
            ValueType::Dict => {
                let Some(dict) = value.as_dict() else {
                    unreachable!()
                };
                let is_wide = dict.is_wide();
                self.begin_dict();
                for elem in dict {
                    let key = if elem.key.value_type() == ValueType::Pointer {
                        unsafe {
                            ValuePointer::from_value(elem.key)
                                .deref_unchecked(is_wide)
                                .to_str()
                        }
                    } else {
                        elem.key.to_str()
                    };
                    self.write_key(key)?;
                    self.write_fleece(elem.val)?;
                }
                self.end_dict()
            }
            ValueType::Pointer => unsafe {
                self.write_fleece(ValuePointer::from_value(value).deref_unchecked(false))
            },
        }
    }

    pub fn begin_array(&mut self, capacity: usize) {
        self.collection_stack.push_array(capacity);
    }

    /// ## Errors
    /// - If there is no open collection.
    /// - If the top open collection is not an Array.
    pub fn end_array(&mut self) -> Result<()> {
        let Some(Collection::Array(mut array)) = self.collection_stack.pop() else {
            return Err(EncodeError::ArrayNotOpen);
        };
        let is_wide = Encoder::<W>::_array_should_be_wide(&array);

        // Write the Array header via `Encodable` trait
        let offset = self._write(&array, is_wide)?;

        self._fix_array_pointers(&mut array, is_wide);

        for v in &array.values {
            self._write(v, is_wide)?;
        }

        self._finished_collection(offset);

        Ok(())
    }

    pub fn begin_dict(&mut self) {
        self.collection_stack.push_dict();
    }

    /// This *MUST* follow the implementation at [`value::Value::dict_key_cmp`]
    pub(crate) fn dict_key_cmp(value1: &DictKey, value2: &DictKey) -> Ordering {
        match (value1, value2) {
            // Inline strings
            (DictKey::Inline(value1), DictKey::Inline(value2)) => {
                value1.as_value().to_str().cmp(value2.as_value().to_str())
            }
            // Pointers to strings
            (DictKey::Pointer(val1, _), DictKey::Pointer(val2, _)) => {
                val1.as_ref().cmp(val2.as_ref())
            }
            (DictKey::Inline(value1), DictKey::Pointer(val2, _)) => {
                value1.as_value().to_str().cmp(val2.as_ref())
            }
            (DictKey::Pointer(val1, _), DictKey::Inline(value2)) => {
                val1.as_ref().cmp(value2.as_value().to_str())
            }
            // SharedKeys
            (DictKey::Shared(value1), DictKey::Shared(value2)) => value1.cmp(value2),
            // SharedKeys are sorted first in the dict
            (DictKey::Shared(_), _) => Ordering::Less,
            (_, DictKey::Shared(_)) => Ordering::Greater,
        }
    }

    /// End the top open Dict. This will write all the Dict's keys and values to the Encoder's 
    /// output.
    /// ## Errors
    /// - If the top open collection is not a Dict.
    /// - If the open Dict has a key with no value.
    pub fn end_dict(&mut self) -> Result<()> {
        match self.collection_stack.top() {
            // Can only end a dict if the top collection is a dict
            Some(Collection::Dict(dict)) => {
                // That dict must not have a key waiting for a value
                if dict.next_key.is_some() {
                    return Err(EncodeError::DictWaitingForValue);
                }
            }
            _ => return Err(EncodeError::DictNotOpen),
        }
        let Some(Collection::Dict(mut dict)) = self.collection_stack.pop() else {
            unreachable!()
        };

        let is_wide = self._dict_should_be_wide(&dict);

        // Write the Dict header via `Encodable` trait
        let offset = self._write(&dict, is_wide)?;

        dict.values
            .sort_unstable_by(|elem1, elem2| Encoder::<W>::dict_key_cmp(&elem1.key, &elem2.key));

        self._fix_dict_pointers(&mut dict, is_wide);

        for elem in &dict.values {
            match &elem.key {
                DictKey::Inline(val) => self._write(val, is_wide)?,
                DictKey::Shared(int_key) => {
                    // Unwrap is safe because u16::to_value only fails if > 2047, that's never the case for shared keys
                    let Some(val) = int_key.to_sized_value() else {
                        unreachable!()
                    };
                    self._write(&val, is_wide)?
                }
                DictKey::Pointer(_, offset) => {
                    if is_wide {
                        let Some(val) = SizedValue::new_pointer(*offset) else {
                            return Err(EncodeError::PointerTooLarge)
                        };
                        self._write(&val, is_wide)?
                    } else {
                        #[allow(clippy::cast_possible_truncation)]
                        let val = SizedValue::new_narrow_pointer(*offset as u16);
                        self._write(&val, is_wide)?
                    }
                }
            };
            self._write(&elem.val, is_wide)?;
        }

        #[allow(clippy::cast_possible_truncation)]
        self._finished_collection(offset);

        Ok(())
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

impl<W: Write> Encoder<W>
where
    Arc<[u8]>: From<W>,
{
    /// Finish encoding and allocate the result in a new `Scope`. That Scope will hold the
    /// `SharedKeys` this encoder held (if any).
    pub fn finish_scoped(mut self) -> Option<Arc<Scope>> {
        self._end();
        self.out.flush().ok();
        let out = self.out;
        let shared_keys = self.shared_keys.map(Arc::new);
        Scope::new_alloced(out, shared_keys)
    }
}

// private
impl<W: Write> Encoder<W> {
    // Always use this function to write values to the output buffer, because it makes sure all values
    // are evenly aligned.
    /// Write a value to the output buffer and return the offset at which it was written.
    /// The offset can be used to create a pointer to the value.
    fn _write<T: Encodable + ?Sized>(&mut self, value: &T, is_wide: bool) -> Result<u32> {
        #[allow(clippy::cast_possible_truncation)]
        let offset = self.len as u32;
        self.len += value
            .write_fleece_to(&mut self.out, is_wide)
            .map_err(|e| EncodeError::Io { source: e })?;
        // Pad to even
        if self.len % 2 != 0 {
            self.out
                .write_all(&[0])
                .map_err(|e| EncodeError::Io { source: e })?;
            self.len += 1;
        }

        Ok(offset)
    }

    /// If we have shared keys, try to encode the key using those. Otherwise, write the key as a
    /// pointer.
    fn _write_key(&mut self, key: &str) -> Result<()> {
        if let Some(shared_keys) = &mut self.shared_keys {
            let Some(Collection::Dict(dict)) = self.collection_stack.top_mut() else {
                return Err(EncodeError::DictNotOpen);
            };
            // If we have shared keys, insert the key into the shared keys and add the corresponding int key to the Dict
            let Some(int_key) = shared_keys.encode_and_insert(key) else {
                return self._write_key_nonshared(key);
            };
            // Unwrap is safe here because `u16::to_value` will only fail if the value is > 2047, which it will never
            // be because the shared_keys holds max 2048 keys (and the first one is 0)
            dict.push_key(DictKey::Shared(int_key))
                .ok_or_else(|| EncodeError::DictWaitingForValue)
        } else {
            self._write_key_nonshared(key)
        }
    }

    fn _write_key_narrow(&mut self, val: SizedValue) -> Result<()> {
        let Some(Collection::Dict(dict)) = self.collection_stack.top_mut() else {
            return Err(EncodeError::DictNotOpen);
        };
        // If the key is small enough to fit in a fixed-width value, inline it
        dict.push_key(DictKey::Inline(val))
            .ok_or_else(|| EncodeError::DictWaitingForValue)
    }

    fn _write_key_nonshared(&mut self, key: &str) -> Result<()> {
        // If we don't have shared keys, write the key to the output buffer and add a pointer to it in the Dict
        let offset = self._write(key, false)?;
        let Some(Collection::Dict(dict)) = self.collection_stack.top_mut() else {
            return Err(EncodeError::DictNotOpen);
        };
        dict.push_key(DictKey::Pointer(key.into(), offset))
            .ok_or_else(|| EncodeError::DictWaitingForValue)
    }

    /// Push a fixed-width Fleece value to the collection which is currently at the top of the stack.
    /// This function will return [`None`] if the top of the stack is not a collection, or if it is a
    /// [`Collection::Dict`] and the last addition was not a key.
    fn _push(&mut self, value: SizedValue) -> Result<()> {
        match self
            .collection_stack
            .top_mut()
            .ok_or_else(|| EncodeError::CollectionNotOpen)?
        {
            Collection::Array(arr) => {
                arr.push(value);
                Ok(())
            }
            Collection::Dict(dict) => dict
                .push_value(value)
                .ok_or_else(|| EncodeError::DictWaitingForKey),
        }
    }

    /// Close all open collections, discard any dangling keys
    fn _end(&mut self) {
        while let Some(collection) = self.collection_stack.top_mut() {
            match collection {
                Collection::Array(_) => self.end_array().ok(),
                Collection::Dict(dict) => {
                    dict.next_key.take();
                    self.end_dict().ok()
                }
            };
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn _actual_pointer_offset(&self, offset_from_start: u32) -> u32 {
        self.len as u32 - offset_from_start
    }

    fn _array_should_be_wide(array: &value_stack::Array) -> bool {
        for v in &array.values {
            if v.is_wide() {
                return true;
            }
        }
        false
    }

    // Only Pointer might require more than 2 bytes, if any do then the whole dict needs to be wide
    fn _dict_should_be_wide(&self, dict: &value_stack::Dict) -> bool {
        let mut len = self.len;
        for elem in &dict.values {
            if let DictKey::Pointer(_, offset) = &elem.key {
                let offset = len - *offset as usize;
                if len - offset > pointer::MAX_NARROW as usize {
                    return true;
                }
            }
            if elem.val.value_type() == ValueType::Pointer && elem.val.is_wide() {
                return true;
            }
            len += 2;
        }
        false
    }

    fn _fix_array_pointers(&self, array: &mut value_stack::Array, is_wide: bool) {
        #[allow(clippy::cast_possible_truncation)]
        let mut len = self.len as u32;
        for elem in &mut array.values {
            if elem.value_type() == ValueType::Pointer {
                let pointer = Encoder::<W>::_fix_pointer(elem, len, is_wide);
                *elem = pointer;
            }
            len += if is_wide { 4 } else { 2 };
        }
    }

    fn _fix_dict_pointers(&self, dict: &mut value_stack::Dict, is_wide: bool) {
        #[allow(clippy::cast_possible_truncation)]
        let mut len = self.len as u32;
        for elem in &mut dict.values {
            if let DictKey::Pointer(_, offset) = &mut elem.key {
                *offset = len - *offset;
            }
            len += if is_wide { 4 } else { 2 };
            if elem.val.value_type() == ValueType::Pointer {
                elem.val = Encoder::<W>::_fix_pointer(&elem.val, len, is_wide);
            }
            len += if is_wide { 4 } else { 2 };
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn _fix_pointer(pointer: &SizedValue, len: u32, is_wide: bool) -> SizedValue {
        // Make sure pointers don't get truncated
        let pointer = if is_wide {
            pointer.clone()
        } else {
            pointer.narrow_pointer()
        };

        let pointer = ValuePointer::from_value(pointer.as_value());

        let offset_from_start = unsafe { pointer.get_offset(is_wide) } as u32;
        let offset = len - offset_from_start;
        if is_wide {
            SizedValue::new_pointer(offset).unwrap()
        } else {
            SizedValue::new_narrow_pointer(offset as u16)
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
            #[allow(clippy::cast_possible_truncation)]
            let root = SizedValue::new_narrow_pointer(
                self._actual_pointer_offset(offset_from_start) as u16,
            );
            self._write(&root, false)
                .expect("IO Error while attempting to write root value");
        }
    }
}
