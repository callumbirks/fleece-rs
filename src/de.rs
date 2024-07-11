use crate::value::array;
use crate::value::pointer::Pointer;
use crate::{Array, Dict, Error, Result, Value, ValueType};
use serde::de::{DeserializeSeed, Visitor};
use serde::{de, forward_to_deserialize_any};

pub struct Deserializer<'value> {
    value: &'value Value,
}

pub fn from_bytes<'a, T>(bytes: &'a [u8]) -> Result<T>
where
    T: serde::Deserialize<'a>,
{
    let value = Value::from_bytes(bytes)?;
    let deserializer = Deserializer::new(value, false);
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
    #[error("Invalid layout for Enum / Variant {0:?}")]
    InvalidEnumLayout(Box<Array>),
}

impl<'value> Deserializer<'value> {
    fn new(value: &'value Value, is_wide: bool) -> Self {
        let value = if value.value_type() == ValueType::Pointer {
            unsafe { Pointer::from_value(value).deref_unchecked(is_wide) }
        } else {
            value
        };
        Self { value }
    }
}

impl<'de, 'value> de::Deserializer<'de> for &Deserializer<'value> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.value.value_type() {
            ValueType::Null => visitor.visit_none(),
            ValueType::Undefined => visitor.visit_unit(),
            ValueType::False | ValueType::True => visitor.visit_bool(self.value.to_bool()),
            ValueType::Short => visitor.visit_i16(self.value.to_short()),
            ValueType::Int => visitor.visit_i64(self.value.to_int()),
            ValueType::UnsignedInt => visitor.visit_u64(self.value.to_unsigned_int()),
            ValueType::Float => visitor.visit_f32(self.value.to_float()),
            ValueType::Double32 | ValueType::Double64 => visitor.visit_f64(self.value.to_double()),
            ValueType::String => visitor.visit_str(self.value.to_str()),
            ValueType::Data => visitor.visit_bytes(self.value.to_data()),
            ValueType::Array => visitor.visit_seq(ArrayAccess::new(Array::from_value(self.value))),
            ValueType::Dict => visitor.visit_map(DictAccess::new(Dict::from_value(self.value))),
            ValueType::Pointer => Err(Error::Deserialize(
                DeserializeError::CannotDeserializePointer,
            )),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> std::result::Result<V::Value, Self::Error> where V: Visitor<'de> {
        match self.value.value_type() { 
            ValueType::Null => visitor.visit_none(),
            _ => visitor.visit_some(self)
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(arr) = self.value.as_array() {
            visitor.visit_seq(ArrayAccess::new(arr))
        } else {
            Err(Error::Deserialize(DeserializeError::NotArray))
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(dict) = self.value.as_dict() {
            visitor.visit_map(DictAccess::new(dict))
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

        visitor.visit_enum(EnumAccess::new(array))
    }

    fn is_human_readable(&self) -> bool {
        false
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char tuple string bytes byte_buf
        unit unit_struct newtype_struct str tuple_struct identifier ignored_any
    }
}

struct ArrayAccess<'iter> {
    iter: array::Iter<'iter>,
}

impl<'iter> ArrayAccess<'iter> {
    fn new(array: &'iter Array) -> Self {
        Self { iter: array.iter() }
    }
}

impl<'iter, 'de> de::SeqAccess<'de> for ArrayAccess<'iter> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            None => Ok(None),
            Some(next) => seed
                .deserialize(&Deserializer::new(next, self.iter.width == 4))
                .map(Some),
        }
    }
}

struct DictAccess<'iter> {
    iter: array::Iter<'iter>,
}

impl<'iter> DictAccess<'iter> {
    fn new(dict: &'iter Dict) -> Self {
        Self {
            iter: dict.array.iter(),
        }
    }
}

impl<'iter, 'de> de::MapAccess<'de> for DictAccess<'iter> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            None => Ok(None),
            Some(next) => seed
                .deserialize(&Deserializer::new(next, self.iter.width == 4))
                .map(Some),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            None => Err(Error::Deserialize(DeserializeError::KeyWithoutValue)),
            Some(next) => seed.deserialize(&Deserializer::new(next, self.iter.width == 4)),
        }
    }
}

struct EnumAccess<'arr> {
    array: &'arr Array,
}

impl<'arr> EnumAccess<'arr> {
    fn new(array: &'arr Array) -> Self {
        Self { array }
    }
}

impl<'arr, 'de> de::EnumAccess<'de> for EnumAccess<'arr> {
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
                    self.array.clone_box(),
                )))?;

        let value = seed.deserialize(&Deserializer::new(variant, self.array.is_wide()))?;

        Ok((value, self))
    }
}

impl<'arr, 'de> de::VariantAccess<'de> for EnumAccess<'arr> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        if self.array.len() == 1 {
            Ok(())
        } else {
            Err(Error::Deserialize(DeserializeError::InvalidEnumLayout(
                self.array.clone_box(),
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
                    self.array.clone_box(),
                )))?;
        seed.deserialize(&Deserializer::new(inner, self.array.is_wide()))
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
                    self.array.clone_box(),
                )))?;
        if let Some(array) = inner.as_array() {
            if array.len() == len {
                return de::Deserializer::deserialize_seq(
                    &Deserializer::new(inner, self.array.is_wide()),
                    visitor,
                );
            }
        }
        Err(Error::Deserialize(DeserializeError::InvalidEnumLayout(
            self.array.clone_box(),
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
                    self.array.clone_box(),
                )))?;
        if let Some(dict) = inner.as_dict() {
            if dict.len() == fields.len() && fields.iter().all(|field| dict.contains_key(field)) {
                return de::Deserializer::deserialize_map(
                    &Deserializer::new(inner, self.array.is_wide()),
                    visitor,
                );
            }
        }
        Err(Error::Deserialize(DeserializeError::InvalidEnumLayout(
            self.array.clone_box(),
        )))
    }
}
