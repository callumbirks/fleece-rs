use crate::encoder::value_stack;
use crate::encoder::{Encodable, NullValue, UndefinedValue};
use crate::value::SizedValue;
use crate::value::{array, varint};
use crate::{value, Value};
use std::io::{Result, Write};

/// All the built-in implementations of [`Encodable`].

impl Encodable for i64 {
    #[allow(clippy::cast_possible_truncation)]
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        // If this is small enough, store it as a short
        if *self <= 2047 || *self >= -2048 {
            #[allow(clippy::cast_possible_truncation)]
            return (*self as i16).write_fleece_to(writer, is_wide);
        }
        let mut buf = [0_u8; 9];
        let byte_count = self.fleece_size() - 1;
        buf[0] = value::tag::INT | ((byte_count as u8) - 1);
        buf[1..].copy_from_slice(&self.to_le_bytes());
        writer.write_all(&buf[..=byte_count])?;
        Ok(byte_count + 1)
    }

    fn fleece_size(&self) -> usize {
        if *self <= 2047 || *self >= -2048 {
            return 2;
        }
        if *self >= 0 {
            8 - self.trailing_zeros() as usize + 1
        } else {
            8 - self.trailing_ones() as usize + 1
        }
    }
    #[allow(clippy::cast_possible_truncation)]
    fn to_sized_value(&self) -> Option<SizedValue> {
        if *self <= 2047 || *self >= -2048 {
            (*self as i16).to_sized_value()
        } else {
            None
        }
    }
}

impl Encodable for u64 {
    #[allow(clippy::cast_possible_truncation)] // Suppress warning for `byte_count as u8`
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        // If this is small enough, store it as a short
        if *self <= 2047 {
            #[allow(clippy::cast_possible_truncation)]
            return (*self as u16).write_fleece_to(writer, is_wide);
        }
        let mut buf = [0_u8; 9];
        let byte_count = self.fleece_size() - 1;
        buf[0] = value::tag::INT | value::extra_flags::UNSIGNED_INT | ((byte_count as u8) - 1);
        buf[1..].copy_from_slice(&self.to_le_bytes());
        writer.write_all(&buf[..=byte_count])?;
        Ok(byte_count + 1)
    }

    fn fleece_size(&self) -> usize {
        if *self <= 2047 {
            2
        } else {
            let trailing_zeros = (self.trailing_zeros() + 7) / 8;
            8 - trailing_zeros as usize + 1
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn to_sized_value(&self) -> Option<SizedValue> {
        if *self <= 2047 {
            (*self as u16).to_sized_value()
        } else {
            None
        }
    }
}

impl Encodable for i32 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        i64::from(*self).write_fleece_to(writer, is_wide)
    }

    fn fleece_size(&self) -> usize {
        i64::from(*self).fleece_size()
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        i64::from(*self).to_sized_value()
    }
}

impl Encodable for u32 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        u64::from(*self).write_fleece_to(writer, is_wide)
    }

    fn fleece_size(&self) -> usize {
        u64::from(*self).fleece_size()
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        u64::from(*self).to_sized_value()
    }
}

impl Encodable for u16 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        // Short can only be 12 bits
        if *self > 2047 {
            return u64::from(*self).write_fleece_to(writer, is_wide);
        }
        let Some(val) = self.to_sized_value() else {
            unreachable!();
        };
        val.write_fleece_to(writer, is_wide)
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        if *self > 2047 {
            return None;
        }
        let mut bytes = self.to_be_bytes();
        bytes[0] |= value::tag::SHORT;
        Some(SizedValue::from_narrow(bytes))
    }
}

impl Encodable for i16 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        // Short can only be 12 bits
        if *self > 2047 || *self < -2048 {
            return i64::from(*self).write_fleece_to(writer, is_wide);
        }
        let Some(val) = self.to_sized_value() else {
            unreachable!();
        };
        val.write_fleece_to(writer, is_wide)
    }

    fn fleece_size(&self) -> usize {
        2
    }
    fn to_sized_value(&self) -> Option<SizedValue> {
        if *self > 2047 || *self < -2048 {
            return None;
        }
        let mut bytes = self.to_be_bytes();
        // Make sure to zero out the top 4 bits (where the tag goes) in-case of sign extension
        bytes[0] = (bytes[0] & 0x0F) | value::tag::SHORT;
        Some(SizedValue::from_narrow(bytes))
    }
}

impl Encodable for u8 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        (*self as u16).write_fleece_to(writer, is_wide)
    }

    fn fleece_size(&self) -> usize {
        (*self as u16).fleece_size()
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        (*self as u16).to_sized_value()
    }
}

impl Encodable for i8 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        (*self as i16).write_fleece_to(writer, is_wide)
    }

    fn fleece_size(&self) -> usize {
        (*self as i16).fleece_size()
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        (*self as i16).to_sized_value()
    }
}

impl Encodable for f32 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, _is_wide: bool) -> Result<usize> {
        let mut buf = [0_u8; 6];
        buf[0] = value::tag::FLOAT;
        buf[2..6].copy_from_slice(&self.to_le_bytes());
        writer.write_all(&buf)?;
        Ok(6)
    }

    fn fleece_size(&self) -> usize {
        6
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        None
    }
}

impl Encodable for f64 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, _is_wide: bool) -> Result<usize> {
        let mut buf = [0_u8; 10];
        buf[0] = value::tag::FLOAT | 0x08;
        buf[2..10].copy_from_slice(&self.to_le_bytes());
        writer.write_all(&buf)?;
        Ok(10)
    }

    fn fleece_size(&self) -> usize {
        10
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        None
    }
}

fn write_fleece_constant<W: Write>(
    writer: &mut W,
    constant: [u8; 2],
    is_wide: bool,
) -> Result<usize> {
    if is_wide {
        writer.write_all(&constant)?;
        Ok(4)
    } else {
        writer.write_all(&constant)?;
        Ok(2)
    }
}

impl Encodable for bool {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        if *self {
            write_fleece_constant(writer, value::constants::TRUE, is_wide)
        } else {
            write_fleece_constant(writer, value::constants::FALSE, is_wide)
        }
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        if *self {
            Some(SizedValue::from_narrow(value::constants::TRUE))
        } else {
            Some(SizedValue::from_narrow(value::constants::FALSE))
        }
    }
}

impl Encodable for NullValue {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        write_fleece_constant(writer, value::constants::NULL, is_wide)
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        Some(SizedValue::from_narrow(value::constants::NULL))
    }
}

impl Encodable for UndefinedValue {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        write_fleece_constant(writer, value::constants::UNDEFINED, is_wide)
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        Some(SizedValue::from_narrow(value::constants::UNDEFINED))
    }
}

// Data and String are encoded the same in Fleece, apart from the value type tag.
fn write_fleece_string<W: Write, const IS_STR: bool>(
    string: &[u8],
    writer: &mut W,
    is_wide: bool,
) -> Result<usize> {
    let mut buf = [0_u8; 4];
    buf[0] = if IS_STR {
        value::tag::STRING
    } else {
        value::tag::DATA
    };

    match string.len() {
        // If size is 1 or 0, size fits in the tiny value and string fits in the second byte.
        0 => {
            if is_wide {
                writer.write_all(&buf)?;
                Ok(4)
            } else {
                writer.write_all(&buf[..2])?;
                Ok(2)
            }
        }
        1 => {
            buf[0] |= 1;
            buf[1] = string[0];
            if is_wide {
                writer.write_all(&buf)?;
                Ok(4)
            } else {
                writer.write_all(&buf[..2])?;
                Ok(2)
            }
        }
        // If size is up to 0x0E (0x0F is the bit that indicates a varint), we can fit the size in the tiny value.
        #[allow(clippy::cast_possible_truncation)]
        2..=0x0E => {
            buf[0] |= string.len() as u8;
            writer.write_all(&buf[0..1])?;
            writer.write_all(string)?;
            Ok(string.len() + 1)
        }
        // Any larger sizes will store the size as a varint.
        _ => {
            buf[0] |= 0x0F;
            let mut varint_buf = [0_u8; varint::MAX_LEN];
            let varint_size = varint::write(&mut varint_buf, string.len() as u64);
            // Write the tag + tiny (1 byte)
            writer.write_all(&buf[0..1])?;
            // Write the varint
            writer.write_all(&varint_buf[..varint_size])?;
            // Write the string
            writer.write_all(string)?;
            Ok(1 + varint_size + string.len())
        }
    }
}

impl Encodable for [u8] {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        write_fleece_string::<_, false>(self, writer, is_wide)
    }

    fn fleece_size(&self) -> usize {
        match self.len() {
            0 | 1 => 2,
            2..=0x0E => 1 + self.len(),
            _ => 1 + varint::size_required(self.len() as u64) + self.len(),
        }
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        match self.len() {
            0 => Some(SizedValue::from_narrow([value::tag::DATA, 0])),
            1 => Some(SizedValue::from_narrow([value::tag::DATA | 0x01, self[0]])),
            _ => None,
        }
    }
}

impl Encodable for str {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        write_fleece_string::<_, true>(self.as_bytes(), writer, is_wide)
    }

    fn fleece_size(&self) -> usize {
        self.as_bytes().fleece_size()
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        match self.len() {
            0 => Some(SizedValue::from_narrow([value::tag::STRING, 0])),
            1 => Some(SizedValue::from_narrow([
                value::tag::STRING | 0x01,
                self.as_bytes()[0],
            ])),
            _ => None,
        }
    }
}

impl<T> Encodable for Option<T>
where
    T: Encodable,
{
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        match self {
            Some(value) => value.write_fleece_to(writer, is_wide),
            None => NullValue.write_fleece_to(writer, is_wide),
        }
    }

    fn fleece_size(&self) -> usize {
        match self {
            Some(value) => value.fleece_size(),
            None => 2,
        }
    }
    fn to_sized_value(&self) -> Option<SizedValue> {
        match self {
            Some(v) => v.to_sized_value(),
            None => NullValue.to_sized_value(),
        }
    }
}

impl Encodable for SizedValue {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        if is_wide {
            writer.write_all(self.as_bytes())?;
            Ok(4)
        } else {
            writer.write_all(&self.as_bytes()[..2])?;
            Ok(2)
        }
    }

    fn fleece_size(&self) -> usize {
        if self.is_wide() {
            4
        } else {
            2
        }
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        Some(self.clone())
    }
}

fn write_valuestack_collection<W: Write>(
    writer: &mut W,
    tag: u8,
    len: usize,
    is_wide: bool,
) -> Result<usize> {
    let mut buf = [0_u8; 2 + varint::MAX_LEN];
    let written = 2 + if len >= array::VARINT_COUNT as usize {
        varint::write(&mut buf[2..], len as u64)
    } else {
        0
    };
    #[allow(clippy::cast_possible_truncation)]
    let inline_size = len.min(array::VARINT_COUNT as usize) as u16;

    buf[0] = tag | (inline_size >> 8) as u8;
    buf[1] = (inline_size & 0xFF) as u8;

    if is_wide {
        buf[0] |= 0x08;
    }

    writer.write_all(&buf[..written])?;
    Ok(written)
}

// Just write the Array header, not the values
impl Encodable for value_stack::Array {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        write_valuestack_collection(writer, value::tag::ARRAY, self.values.len(), is_wide)
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        None
    }
}

impl Encodable for value_stack::Dict {
    // Just write the Dict header, not the values
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Result<usize> {
        write_valuestack_collection(writer, value::tag::DICT, self.values.len(), is_wide)
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        None
    }
}

pub trait AsBoxedValue {
    /// Encode `self` into Fleece and Box the resulting `Value`, returning it.
    fn as_boxed_value(&self) -> Result<Box<Value>>;
}

impl<T> AsBoxedValue for T
where
    T: Encodable + ?Sized,
{
    fn as_boxed_value(&self) -> Result<Box<Value>> {
        let mut buf = Vec::with_capacity(self.fleece_size());
        self.write_fleece_to(&mut buf, false)?;
        let boxed = buf.into_boxed_slice();
        Ok(unsafe { std::mem::transmute::<Box<[u8]>, Box<Value>>(boxed) })
    }
}
