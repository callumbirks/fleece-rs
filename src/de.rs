use crate::scope::Scope;
use crate::value::array;
use crate::value::pointer::Pointer;
use crate::{Array, Dict, Error, Result, SharedKeys, Value, ValueType};
use serde::de::{DeserializeSeed, Visitor};
use serde::{de, forward_to_deserialize_any};
use std::sync::Arc;

pub struct Deserializer<'value, 'sk> {
    value: &'value Value,
    shared_keys: SK<'sk>,
    is_dict_key: bool,
}

enum SK<'sk> {
    None,
    Ref(&'sk Arc<SharedKeys>),
    Owned(Arc<SharedKeys>),
}

impl<'sk> SK<'sk> {
    fn as_ref(&self) -> SK {
        match self {
            SK::None => SK::None,
            SK::Ref(sk) => SK::Ref(sk),
            SK::Owned(sk) => SK::Ref(sk),
        }
    }

    fn shared_keys(&self) -> Option<&Arc<SharedKeys>> {
        match self {
            SK::None => None,
            SK::Ref(sk) => Some(sk),
            SK::Owned(sk) => Some(sk),
        }
    }
}

/// Deserialize a value from Fleece-encoded bytes.
/// # Errors
/// Returns an error if the bytes are not valid Fleece-encoded data or if the data cannot be
/// deserialized into the requested type.
pub fn from_bytes<'a, T>(bytes: &'a [u8]) -> Result<T>
where
    T: serde::Deserialize<'a>,
{
    let value = Value::from_bytes(bytes)?;
    let deserializer = Deserializer::init(value, false);
    T::deserialize(&deserializer)
}

#[derive(thiserror::Error, Debug)]
pub enum DeserializeError {
    #[error("Cannot deserialize pointer")]
    CannotDeserializePointer,
    #[error("Attempted to deserialize a sequence from a non-Array")]
    NotArray,
    #[error("Attempted to deserialize a map from a non-Dict")]
    NotDict,
    #[error("Invalid Enum, expected Array, found {0:?}")]
    InvalidEnumType(ValueType),
    #[error("Found a Dict Key without Value!")]
    KeyWithoutValue,
    #[error("Invalid layout for Enum / Variant {1:?} for '{0}'")]
    InvalidEnumLayout(&'static str, String),
    #[error("Failed to decode SharedKeys")]
    CannotDecodeSharedKeys,
}

impl<'value, 'sk> Deserializer<'value, 'sk> {
    fn init(value: &'value Value, is_wide: bool) -> Self {
        let sk = match Scope::find_shared_keys(value.bytes.as_ptr()) {
            Some(sk) => SK::Owned(sk),
            None => SK::None,
        };
        Self::new(value, is_wide, sk)
    }

    fn new(value: &'value Value, is_wide: bool, shared_keys: SK<'sk>) -> Self {
        let value = if value.value_type() == ValueType::Pointer {
            unsafe { Pointer::from_value(value).deref_unchecked(is_wide) }
        } else {
            value
        };
        Self {
            value,
            shared_keys,
            is_dict_key: false,
        }
    }

    fn new_for_dict_key(value: &'value Value, is_wide: bool, shared_keys: SK<'sk>) -> Self {
        let value = if value.value_type() == ValueType::Pointer {
            unsafe { Pointer::from_value(value).deref_unchecked(is_wide) }
        } else {
            value
        };
        Self {
            value,
            shared_keys,
            is_dict_key: true,
        }
    }
}

impl<'de, 'value, 'sk> de::Deserializer<'de> for &Deserializer<'value, 'sk> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.value.value_type() {
            ValueType::Null => visitor.visit_none(),
            ValueType::Undefined => visitor.visit_unit(),
            ValueType::False | ValueType::True => visitor.visit_bool(self.value.to_bool()),
            ValueType::Short if self.is_dict_key => {
                let int_key = self.value.to_unsigned_short();
                let Some(str_key) = self
                    .shared_keys
                    .shared_keys()
                    .and_then(|sk| sk.decode(int_key))
                else {
                    return Err(Error::Deserialize(DeserializeError::CannotDecodeSharedKeys));
                };
                visitor.visit_str(str_key)
            }
            ValueType::Short => visitor.visit_i16(self.value.to_short()),
            ValueType::Int => visitor.visit_i64(self.value.to_int()),
            ValueType::UnsignedInt => visitor.visit_u64(self.value.to_unsigned_int()),
            ValueType::Float => visitor.visit_f32(self.value.to_float()),
            ValueType::Double32 | ValueType::Double64 => visitor.visit_f64(self.value.to_double()),
            ValueType::String => visitor.visit_str(self.value.to_str()),
            ValueType::Data => visitor.visit_bytes(self.value.to_data()),
            ValueType::Array => visitor.visit_seq(ArrayAccess::new(
                Array::from_value(self.value),
                self.shared_keys.as_ref(),
            )),
            ValueType::Dict => visitor.visit_map(DictAccess::new(
                Dict::from_value(self.value),
                self.shared_keys.as_ref(),
            )),
            ValueType::Pointer => Err(Error::Deserialize(
                DeserializeError::CannotDeserializePointer,
            )),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> std::result::Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.value.value_type() {
            ValueType::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(arr) = self.value.as_array() {
            visitor.visit_seq(ArrayAccess::new(arr, self.shared_keys.as_ref()))
        } else {
            Err(Error::Deserialize(DeserializeError::NotArray))
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(dict) = self.value.as_dict() {
            visitor.visit_map(DictAccess::new(dict, self.shared_keys.as_ref()))
        } else {
            Err(Error::Deserialize(DeserializeError::NotDict))
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let Some(array) = self.value.as_array() else {
            return Err(Error::Deserialize(DeserializeError::InvalidEnumType(
                self.value.value_type(),
            )));
        };

        visitor.visit_enum(EnumAccess::new(array, self.shared_keys.as_ref()))
    }

    fn is_human_readable(&self) -> bool {
        false
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char tuple string bytes byte_buf
        unit unit_struct newtype_struct str tuple_struct identifier ignored_any
    }
}

struct ArrayAccess<'iter, 'sk> {
    iter: array::Iter<'iter>,
    shared_keys: SK<'sk>,
}

impl<'iter, 'sk> ArrayAccess<'iter, 'sk> {
    fn new(array: &'iter Array, shared_keys: SK<'sk>) -> Self {
        Self {
            iter: array.iter(),
            shared_keys,
        }
    }
}

impl<'iter, 'de, 'sk> de::SeqAccess<'de> for ArrayAccess<'iter, 'sk> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            None => Ok(None),
            Some(next) => seed
                .deserialize(&Deserializer::new(
                    next,
                    self.iter.width == 4,
                    self.shared_keys.as_ref(),
                ))
                .map(Some),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

struct DictAccess<'iter, 'sk> {
    iter: array::Iter<'iter>,
    shared_keys: SK<'sk>,
}

impl<'iter, 'sk> DictAccess<'iter, 'sk> {
    fn new(dict: &'iter Dict, shared_keys: SK<'sk>) -> Self {
        Self {
            iter: dict.array.iter(),
            shared_keys,
        }
    }
}

impl<'iter, 'de, 'sk> de::MapAccess<'de> for DictAccess<'iter, 'sk> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            None => Ok(None),
            Some(next) => seed
                .deserialize(&Deserializer::new_for_dict_key(
                    next,
                    self.iter.width == 4,
                    self.shared_keys.as_ref(),
                ))
                .map(Some),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            None => Err(Error::Deserialize(DeserializeError::KeyWithoutValue)),
            Some(next) => seed.deserialize(&Deserializer::new(
                next,
                self.iter.width == 4,
                self.shared_keys.as_ref(),
            )),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len() / 2)
    }
}

struct EnumAccess<'arr, 'sk> {
    array: &'arr Array,
    shared_keys: SK<'sk>,
}

impl<'arr, 'sk> EnumAccess<'arr, 'sk> {
    fn new(array: &'arr Array, shared_keys: SK<'sk>) -> Self {
        Self { array, shared_keys }
    }
}

impl<'arr, 'sk, 'de> de::EnumAccess<'de> for EnumAccess<'arr, 'sk> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        // The variant index is at array index 0
        let variant =
            self.array
                .get(0)
                .ok_or(Error::Deserialize(DeserializeError::InvalidEnumLayout(
                    "variant seed",
                    format!("{:?}", self.array),
                )))?;

        let value = seed.deserialize(&Deserializer::new(
            variant,
            self.array.is_wide(),
            self.shared_keys.as_ref(),
        ))?;

        Ok((value, self))
    }
}

impl<'arr, 'sk, 'de> de::VariantAccess<'de> for EnumAccess<'arr, 'sk> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        if self.array.len() == 1 {
            Ok(())
        } else {
            Err(Error::Deserialize(DeserializeError::InvalidEnumLayout(
                "unit variant",
                format!("{:?}", self.array),
            )))
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        // Inner variant data is at index 1 in the array
        let inner =
            self.array
                .get(1)
                .ok_or(Error::Deserialize(DeserializeError::InvalidEnumLayout(
                    "newtype variant",
                    format!("{:?}", self.array),
                )))?;
        seed.deserialize(&Deserializer::new(
            inner,
            self.array.is_wide(),
            self.shared_keys.as_ref(),
        ))
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // Inner tuple is stored as an array at index 1
        let inner =
            self.array
                .get(1)
                .ok_or(Error::Deserialize(DeserializeError::InvalidEnumLayout(
                    "tuple variant (no array)",
                    format!("{:?}", self.array),
                )))?;
        if let Some(array) = inner.as_array() {
            if array.len() == len {
                return de::Deserializer::deserialize_seq(
                    &Deserializer::new(inner, self.array.is_wide(), self.shared_keys.as_ref()),
                    visitor,
                );
            }
        }
        Err(Error::Deserialize(DeserializeError::InvalidEnumLayout(
            "tuple variant (invalid array)",
            format!("{:?}", self.array),
        )))
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // Inner struct is stored as a dict at index 1
        let inner =
            self.array
                .get(1)
                .ok_or(Error::Deserialize(DeserializeError::InvalidEnumLayout(
                    "struct variant (no array)",
                    format!("{:?}", self.array),
                )))?;
        if let Some(dict) = inner.as_dict() {
            if dict.len() == fields.len() {
                let correct_keys = if let Some(sk) = self.shared_keys.shared_keys() {
                    fields
                        .iter()
                        .all(|field| dict.contains_key_with_shared_keys(field, sk))
                } else {
                    fields.iter().all(|field| dict.contains_key(field))
                };

                if correct_keys {
                    return de::Deserializer::deserialize_map(
                        &Deserializer::new(inner, self.array.is_wide(), self.shared_keys.as_ref()),
                        visitor,
                    );
                }
            }
        }
        Err(Error::Deserialize(DeserializeError::InvalidEnumLayout(
            "struct variant (invalid dict)",
            format!("{:?}", self.array),
        )))
    }
}
