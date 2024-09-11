use alloc::string::ToString;
use alloc::{sync::Arc, vec::Vec};
use core::fmt;

use serde::ser;
use serde::ser::{Impossible, SerializeMap, SerializeSeq, SerializeTuple};

use crate::encoder::{EncodeError, NullValue, UndefinedValue};
use crate::scope::Scope;
use crate::{Encoder, SharedKeys};
use crate::{Error, Result};

pub struct Serializer {
    encoder: Encoder,
}

/// Serialize the given value into Fleece, and return the encoded
/// bytes in a `Vec`.
/// The `value` parameter must be an enum, sequence, map or non-unit struct.
/// Maps must have string (or char) keys.
/// # Errors
/// - Map keys which are not Strings.
/// - If the `value` is not some sort of enum, sequence, map or non-unit struct.
pub fn to_bytes<T>(value: T) -> Result<Vec<u8>>
where
    T: ser::Serialize,
{
    let mut serializer = Serializer::new();
    match value.serialize(&mut serializer) {
        Ok(()) => Ok(serializer.encoder.finish()),
        Err(Error::Encode(EncodeError::CollectionNotOpen)) => {
            Err(Error::Serialize(SerializeError::ValueNotCollection))
        }
        Err(other) => Err(other),
    }
}

/// Serialize the given value into Fleece, using [`SharedKeys`].
/// Return the encoded bytes wrapped in a [`Scope`].
/// The `value` parameter must be an enum, sequence, map or non-unit struct.
/// Maps must have string (or char) keys.
/// # Errors
/// - Map keys which are not Strings.
/// - If the `value` is not some sort of enum, sequence, map or non-unit struct.
pub fn to_bytes_with_shared_keys<T>(value: T) -> Result<Arc<Scope>>
where
    T: ser::Serialize,
{
    let mut serializer = Serializer::new();
    serializer.set_shared_keys(SharedKeys::new());
    match value.serialize(&mut serializer) {
        Ok(()) => Ok(serializer.encoder.finish_scoped()),
        Err(Error::Encode(EncodeError::CollectionNotOpen)) => {
            Err(Error::Serialize(SerializeError::ValueNotCollection))
        }
        Err(other) => Err(other),
    }
}

#[derive(Debug)]
pub enum SerializeError {
    KeyNotString(KeyType),
    ValueNotCollection,
}

impl fmt::Display for SerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SerializeError::KeyNotString(key_type) => {
                write!(f, "Map keys must be a String, found {key_type:?}")
            }
            SerializeError::ValueNotCollection => write!(
                f,
                "The value parameter must be an enum, sequence, map or non-unit struct"
            ),
        }
    }
}

impl Serializer {
    fn new() -> Self {
        Self {
            encoder: Encoder::new(),
        }
    }

    fn set_shared_keys(&mut self, shared_keys: SharedKeys) {
        self.encoder.set_shared_keys(shared_keys);
    }
}

impl<'ser> serde::Serializer for &'ser mut Serializer {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok> {
        self.encoder.write_value(&v).map_err(Error::Encode)
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok> {
        let str = v.to_string();
        self.encoder
            .write_value(str.as_str())
            .map_err(Error::Encode)
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok> {
        self.encoder.write_value(v).map_err(Error::Encode)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok> {
        self.encoder.write_value(v).map_err(Error::Encode)
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        self.encoder.write_value(&NullValue).map_err(Error::Encode)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::Serialize::serialize(value, self)
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        self.encoder
            .write_value(&UndefinedValue)
            .map_err(Error::Encode)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        self.serialize_none()
    }

    // Array [ VARIANT_NAME ]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok> {
        self.encoder.begin_array(1).map_err(Error::Encode)?;
        self.encoder.write_value(variant).map_err(Error::Encode)?;
        self.encoder.end_array().map_err(Error::Encode)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::Serialize::serialize(value, self)
    }

    // Array [ VARIANT_NAME, VARIANT_DATA ]
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok>
    where
        T: ?Sized + ser::Serialize,
    {
        self.encoder.begin_array(2).map_err(Error::Encode)?;
        self.encoder.write_value(variant).map_err(Error::Encode)?;
        ser::Serialize::serialize(value, &mut *self)?;
        self.encoder.end_array().map_err(Error::Encode)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.encoder
            .begin_array(len.unwrap_or(10))
            .map_err(Error::Encode)?;
        Ok(self)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    // Array [ VARIANT_NAME, Array [ DATA, DATA, DATA, ... ] ]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.encoder.begin_array(3)?;
        self.encoder.write_value(variant).map_err(Error::Encode)?;
        self.encoder.begin_array(len)?;
        Ok(self)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        self.encoder.begin_dict().map_err(Error::Encode)?;
        Ok(self)
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        self.encoder.begin_dict().map_err(Error::Encode)?;
        Ok(self)
    }

    // Array [ VARIANT_NAME, Dict { KEY: VALUE, KEY: VALUE, ... } ]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.encoder.begin_array(len + 2).map_err(Error::Encode)?;
        self.encoder.write_value(variant).map_err(Error::Encode)?;
        self.encoder.begin_dict().map_err(Error::Encode)?;
        Ok(self)
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

impl<'ser> SerializeSeq for &'ser mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::Serialize::serialize(value, &mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        self.encoder.end_array().map_err(Error::Encode)
    }
}

#[derive(Debug)]
pub enum KeyType {
    Bool,
    Int,
    Float,
    Bytes,
    Unit,
    Option,
    Enum,
    Struct,
    Tuple,
    Sequence,
    Map,
}

struct MapKeySerializer<'ser> {
    ser: &'ser mut Serializer,
}

impl<'ser> serde::Serializer for MapKeySerializer<'ser> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    fn serialize_bool(self, _: bool) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Bool,
        )))
    }

    fn serialize_i8(self, _: i8) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(KeyType::Int)))
    }

    fn serialize_i16(self, _: i16) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(KeyType::Int)))
    }

    fn serialize_i32(self, _: i32) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(KeyType::Int)))
    }

    fn serialize_i64(self, _: i64) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(KeyType::Int)))
    }

    fn serialize_u8(self, _: u8) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(KeyType::Int)))
    }

    fn serialize_u16(self, _: u16) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(KeyType::Int)))
    }

    fn serialize_u32(self, _: u32) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(KeyType::Int)))
    }

    fn serialize_u64(self, _: u64) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(KeyType::Int)))
    }

    fn serialize_f32(self, _: f32) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Float,
        )))
    }

    fn serialize_f64(self, _: f64) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Float,
        )))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok> {
        let str = v.to_string();
        self.serialize_str(&str)
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok> {
        Ok(self.ser.encoder.write_key(v)?)
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Bytes,
        )))
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Option,
        )))
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::Serialize::serialize(value, self)
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Unit,
        )))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Unit,
        )))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Enum,
        )))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::Serialize::serialize(value, self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok>
    where
        T: ?Sized + ser::Serialize,
    {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Enum,
        )))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Sequence,
        )))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Tuple,
        )))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Tuple,
        )))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Enum,
        )))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::Serialize(SerializeError::KeyNotString(KeyType::Map)))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Struct,
        )))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::Serialize(SerializeError::KeyNotString(
            KeyType::Enum,
        )))
    }
}

impl<'ser> SerializeMap for &'ser mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::Serialize::serialize(key, MapKeySerializer { ser: self })
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::Serialize::serialize(value, &mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(self.encoder.end_dict()?)
    }
}

impl<'ser> SerializeTuple for &'ser mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl<'ser> ser::SerializeTupleStruct for &'ser mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok> {
        ser::SerializeSeq::end(self)
    }
}

impl<'ser> ser::SerializeTupleVariant for &'ser mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::Serialize::serialize(value, &mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        self.encoder.end_array()?;
        self.encoder.end_array().map_err(Error::Encode)
    }
}

impl<'ser> ser::SerializeStruct for &'ser mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        self.encoder.write_key(key)?;
        ser::Serialize::serialize(value, &mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        self.encoder.end_dict().map_err(Error::Encode)
    }
}

impl<'ser> ser::SerializeStructVariant for &'ser mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::SerializeStruct::serialize_field(self, key, value)
    }

    fn end(self) -> Result<Self::Ok> {
        self.encoder.end_dict().map_err(Error::Encode)?;
        self.encoder.end_array().map_err(Error::Encode)
    }
}
