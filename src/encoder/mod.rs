use core::borrow::Borrow;
use core::cmp::Ordering;
use core::num::NonZeroUsize;

use crate::encoder::value_stack::{Collection, CollectionStack, DictKey};
use crate::scope::Scope;
use crate::value::pointer::Pointer as ValuePointer;
use crate::value::SizedValue;
use crate::value::{pointer, ValueType};
use crate::{value, SharedKeys, Value};
use error::Result;

use alloc::{sync::Arc, vec::Vec};

mod encodable;
mod error;
mod value_stack;

use crate::alloced::AllocedValue;
pub(crate) use encodable::AsBoxedValue;
pub use error::EncodeError;

pub struct NullValue;
pub struct UndefinedValue;

mod private {
    pub trait Sealed {}
}

// Implementations are in the `encodable` module
/// This trait is required for a value to be written to the `Encoder`.
pub trait Encodable: private::Sealed {
    /// Write self to the given writer, encoded as Fleece. Return [`None`] if any write operations fail.
    /// Return [`Some`] with the number of bytes written if the value was written successfully.
    /// # Errors
    /// Any IO errors produced by the `writer`.
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize>;
    /// The number of bytes necessary to encode this value in Fleece.
    fn fleece_size(&self) -> usize;
    /// Construct a [`SizedValue`] from this value, if this value can be represented in 2 bytes of Fleece. Otherwise,
    /// return [`None`].
    /// Use [`SizedValue::from_narrow`] to construct the value.
    fn to_sized_value(&self) -> Option<SizedValue>;
}

#[derive(Default)]
pub struct Encoder {
    out: Vec<u8>,
    shared_keys: Option<SharedKeys>,
    collection_stack: CollectionStack,
    top_collection_closed: bool,
}

impl Encoder {
    #[must_use]
    pub fn new() -> Encoder {
        Self::default()
    }

    /// A convenience function which is the same as [`Encoder::finish`], but returns an
    /// [`AllocedValue`].
    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn finish_value(self) -> AllocedValue {
        let vec = self.finish();
        #[cfg(not(debug_assertions))]
        unsafe {
            Value::clone_from_bytes_unchecked(&vec)
        }
        #[cfg(debug_assertions)]
        Value::clone_from_bytes(&vec).unwrap()
    }

    #[must_use]
    pub fn new_to_writer(out: Vec<u8>) -> Self {
        Self {
            out,
            shared_keys: None,
            collection_stack: CollectionStack::new(),
            top_collection_closed: false,
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
            self._write_key_inline(val)
        } else {
            // Other keys are written either as a pointer or using SharedKeys, if available.
            self._write_key(key)
        }
    }

    /// Write an [`Encodable`] type to the encoder. The parameter may be any borrowed form of an Encodable type.
    /// `R: Borrow<T>` enables us to pass something like a Rc<T> directly to this function
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
            let offset = self._write(value, false, false);
            let pointer = SizedValue::new_pointer(offset).ok_or(EncodeError::PointerTooLarge)?;
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
    pub fn write_fleece(&mut self, value: &Value) -> Result<()> {
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
                self.begin_array(array.len())?;
                for val in array {
                    self.write_fleece(val)?;
                }
                self.end_array()
            }
            ValueType::Dict => {
                let Some(dict) = value.as_dict() else {
                    unreachable!()
                };
                self.begin_dict()?;
                for (key, value) in dict {
                    self.write_key(key)?;
                    self.write_fleece(value)?;
                }
                self.end_dict()
            }
            ValueType::Pointer => unsafe {
                self.write_fleece(ValuePointer::from_value(value).deref_unchecked(false))
            },
        }
    }

    pub fn set_shared_keys(&mut self, shared_keys: SharedKeys) {
        self.shared_keys = Some(shared_keys);
    }

    /// # Errors
    /// - If the top-level collection is a Dict and is waiting for a key.
    /// - If the top-level collection has already been closed.
    pub fn begin_array(&mut self, capacity: usize) -> Result<()> {
        if self.top_collection_closed {
            return Err(EncodeError::MultiTopLevelCollection);
        }
        self.collection_stack.push_array(capacity)
    }

    /// ## Errors
    /// - If there is no open collection.
    /// - If the top open collection is not an Array.
    pub fn end_array(&mut self) -> Result<()> {
        let Some(Collection::Array(mut array)) = self.collection_stack.pop() else {
            return Err(EncodeError::ArrayNotOpen);
        };
        let is_wide = self._array_should_be_wide(&array);

        // Write the Array header via `Encodable` trait
        let offset = self._write(&array, is_wide, true);

        self._fix_array_pointers(&mut array, is_wide);

        for v in &array.values {
            self._write(v, is_wide, false);
        }

        self._finished_collection(offset)?;

        Ok(())
    }

    /// # Errors
    /// - If the top-level collection is a Dict and is waiting for a key.
    /// - If the top-level collection is already closed.
    pub fn begin_dict(&mut self) -> Result<()> {
        if self.top_collection_closed {
            return Err(EncodeError::CollectionNotOpen);
        }
        self.collection_stack.push_dict()
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
        let offset = self._write(&dict, is_wide, true);

        dict.values
            .sort_unstable_by(|elem1, elem2| Encoder::dict_key_cmp(&elem1.key, &elem2.key));

        self._fix_dict_pointers(&mut dict, is_wide);

        for elem in &dict.values {
            match &elem.key {
                DictKey::Inline(val) => self._write(val, is_wide, false),
                DictKey::Shared(int_key) => {
                    let Some(val) = int_key.to_sized_value() else {
                        unreachable!()
                    };
                    self._write(&val, is_wide, false)
                }
                DictKey::Pointer(_, offset) => {
                    if is_wide {
                        let Some(val) = SizedValue::new_wide_pointer(*offset) else {
                            return Err(EncodeError::PointerTooLarge);
                        };
                        self._write(&val, is_wide, false)
                    } else {
                        #[allow(clippy::cast_possible_truncation)]
                        let Some(val) = SizedValue::new_narrow_pointer(*offset as u16) else {
                            return Err(EncodeError::PointerTooLarge);
                        };
                        self._write(&val, is_wide, false)
                    }
                }
            };
            self._write(&elem.val, is_wide, false);
        }

        #[allow(clippy::cast_possible_truncation)]
        self._finished_collection(offset)?;

        Ok(())
    }

    pub fn finish(mut self) -> Vec<u8> {
        self._end();
        self.out
    }

    pub fn finish_scoped(mut self) -> Arc<Scope> {
        self._end();
        let shared_keys = self.shared_keys.map(Arc::new);
        Scope::new(self.out, shared_keys)
    }

    /// This *MUST* follow the implementation at [`Value::dict_key_cmp`]
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
}

impl Encoder {
    // Always use this function to write values to the output buffer, because it makes sure all values
    // are evenly aligned.
    /// Write a value to the output buffer and return the offset at which it was written.
    /// The offset can be used to create a pointer to the value.
    fn _write<T: Encodable + ?Sized>(
        &mut self,
        value: &T,
        is_wide: bool,
        is_collection: bool,
    ) -> u32 {
        let offset = self.out.len();
        let size_required = if is_wide && !is_collection {
            value.fleece_size().max(4)
        } else {
            value.fleece_size()
        };
        self.out.extend(core::iter::repeat(0).take(size_required));
        let written =
            value.write_fleece_to(&mut self.out[offset..(offset + size_required)], is_wide);
        assert_eq!(
            written,
            Some(unsafe { NonZeroUsize::new_unchecked(size_required) })
        );
        // Pad to even
        if self.out.len() % 2 != 0 {
            self.out.push(0);
        }

        #[allow(clippy::cast_possible_truncation)]
        {
            offset as u32
        }
    }

    fn _write_key_inline(&mut self, val: SizedValue) -> Result<()> {
        let Some(Collection::Dict(dict)) = self.collection_stack.top_mut() else {
            return Err(EncodeError::DictNotOpen);
        };
        // If the key is small enough to fit in a fixed-width value, inline it
        dict.push_key(DictKey::Inline(val))
            .ok_or(EncodeError::DictWaitingForValue)
    }

    fn _write_key(&mut self, key: &str) -> Result<()> {
        if let Some(shared_keys) = &mut self.shared_keys {
            let Some(Collection::Dict(dict)) = self.collection_stack.top_mut() else {
                return Err(EncodeError::DictNotOpen);
            };
            // If we have shared keys, insert the key into it and add the corresponding int key to the Dict
            let Some(int_key) = shared_keys.encode_and_insert(key) else {
                return self._write_key_pointer(key);
            };
            dict.push_key(DictKey::Shared(int_key))
                .ok_or(EncodeError::DictWaitingForValue)
        } else {
            self._write_key_pointer(key)
        }
    }

    fn _write_key_pointer(&mut self, key: &str) -> Result<()> {
        // If we don't have shared keys, write the key to the output buffer and add a pointer to it in the Dict
        let offset = self._write(key, false, false);
        let Some(Collection::Dict(dict)) = self.collection_stack.top_mut() else {
            return Err(EncodeError::DictNotOpen);
        };
        dict.push_key(DictKey::Pointer(key.into(), offset))
            .ok_or(EncodeError::DictWaitingForValue)
    }

    /// Push a fixed-width Fleece value to the collection which is currently at the top of the stack.
    /// This function will return [`None`] if the top of the stack is not a collection, or if it is a
    /// [`Collection::Dict`] and the last addition was not a key.
    fn _push(&mut self, value: SizedValue) -> Result<()> {
        match self
            .collection_stack
            .top_mut()
            .ok_or(EncodeError::CollectionNotOpen)?
        {
            Collection::Array(arr) => {
                arr.push(value);
                Ok(())
            }
            Collection::Dict(dict) => dict.push_value(value).ok_or(EncodeError::DictWaitingForKey),
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
        self.out.len() as u32 - offset_from_start
    }

    fn _array_should_be_wide(&self, array: &value_stack::Array) -> bool {
        for v in &array.values {
            if v.value_type() == ValueType::Pointer
                && v.actual_offset(self.out.len()) > u32::from(pointer::MAX_NARROW)
            {
                return true;
            }
        }
        false
    }

    // Only Pointer might require more than 2 bytes, if any do then the whole dict needs to be wide
    fn _dict_should_be_wide(&self, dict: &value_stack::Dict) -> bool {
        let mut len = self.out.len();
        for elem in &dict.values {
            if let DictKey::Pointer(_, offset) = &elem.key {
                let offset = len - *offset as usize;
                if len - offset > pointer::MAX_NARROW as usize {
                    return true;
                }
            }
            if elem.val.value_type() == ValueType::Pointer
                && elem.val.actual_offset(self.out.len()) > u32::from(pointer::MAX_NARROW)
            {
                return true;
            }
            len += 2;
        }
        false
    }

    fn _fix_array_pointers(&self, array: &mut value_stack::Array, is_wide: bool) {
        #[allow(clippy::cast_possible_truncation)]
        let mut len = self.out.len() as u32;
        for elem in &mut array.values {
            if elem.value_type() == ValueType::Pointer {
                let pointer = Encoder::_fix_pointer(elem, len, is_wide);
                *elem = pointer;
            }
            len += if is_wide { 4 } else { 2 };
        }
    }

    fn _fix_dict_pointers(&self, dict: &mut value_stack::Dict, is_wide: bool) {
        #[allow(clippy::cast_possible_truncation)]
        let mut len = self.out.len() as u32;
        for elem in &mut dict.values {
            if let DictKey::Pointer(_, offset) = &mut elem.key {
                *offset = len - *offset;
            }
            len += if is_wide { 4 } else { 2 };
            if elem.val.value_type() == ValueType::Pointer {
                elem.val = Encoder::_fix_pointer(&elem.val, len, is_wide);
            }
            len += if is_wide { 4 } else { 2 };
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn _fix_pointer(sized_pointer: &SizedValue, len: u32, is_wide: bool) -> SizedValue {
        let pointer = ValuePointer::from_value(sized_pointer.as_value());

        let offset_from_start = unsafe { pointer.get_offset(sized_pointer.is_wide()) };
        let offset = len - offset_from_start;
        if is_wide {
            SizedValue::new_wide_pointer(offset).expect("Pointer unexpectedly large")
        } else {
            SizedValue::new_narrow_pointer(offset as u16).expect("Pointer unexpectedly large")
        }
    }

    fn _finished_collection(&mut self, offset_from_start: u32) -> Result<()> {
        if let Some(collection) = self.collection_stack.top_mut() {
            let pointer =
                SizedValue::new_pointer(offset_from_start).ok_or(EncodeError::PointerTooLarge)?;
            match collection {
                Collection::Dict(dict) => {
                    dict.push_value(pointer);
                }
                Collection::Array(array) => {
                    array.push(pointer);
                }
            }
        } else {
            // The last collection is finished, write the root value at the end.
            // This root value points to the outermost collection.
            let offset = self._actual_pointer_offset(offset_from_start);
            #[allow(clippy::cast_possible_truncation)]
            let root = if offset <= u32::from(pointer::MAX_NARROW) {
                SizedValue::new_narrow_pointer(offset as u16).unwrap()
            } else {
                // The root value must be 2 bytes, so if the pointer to the top-level collection
                // is 4 bytes wide, we need to write that, then write another 2-byte pointer to that
                let inner_root =
                    SizedValue::new_pointer(offset).ok_or(EncodeError::PointerTooLarge)?;
                self._write(&inner_root, true, false);
                SizedValue::new_narrow_pointer(4).unwrap()
            };
            self._write(&root, false, false);
            self.top_collection_closed = true;
        }
        Ok(())
    }
}
