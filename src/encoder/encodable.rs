use crate::encoder::{Encodable, NullValue, UndefinedValue};
use crate::raw::sized::{SizedValue};
use crate::raw::{value, varint};
use std::io::Write;

impl Encodable for i64 {
    #[allow(clippy::cast_possible_truncation)]
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
        // If this is small enough, store it as a short
        if *self <= 2047 || *self >= -2048 {
            #[allow(clippy::cast_possible_truncation)]
            return (*self as i16).write_fleece_to(writer, is_wide);
        }
        let mut buf = [0_u8; 9];
        let byte_count = self.fleece_size() - 1;
        buf[0] = value::tag::INT | ((byte_count as u8) - 1);
        buf[1..].copy_from_slice(&self.to_le_bytes());
        writer.write_all(&buf[..=byte_count]).ok()?;
        Some(byte_count + 1)
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
    fn to_value(&self) -> Option<SizedValue> {
        if *self <= 2047 || *self >= -2048 {
            (*self as i16).to_value()
        } else {
            None
        }
    }
}

impl Encodable for u64 {
    #[allow(clippy::cast_possible_truncation)] // Suppress warning for `byte_count as u8`
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
        // If this is small enough, store it as a short
        if *self <= 2047 {
            #[allow(clippy::cast_possible_truncation)]
            return (*self as u16).write_fleece_to(writer, is_wide);
        }
        let mut buf = [0_u8; 9];
        let byte_count = self.fleece_size() - 1;
        buf[0] = value::tag::INT | ((byte_count as u8) - 1);
        buf[1..].copy_from_slice(&self.to_le_bytes());
        writer.write_all(&buf[..=byte_count]).ok()?;
        Some(byte_count + 1)
    }

    fn fleece_size(&self) -> usize {
        if *self <= 2047 {
            2
        } else {
            8 - self.trailing_zeros() as usize + 1
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn to_value(&self) -> Option<SizedValue> {
        if *self <= 2047 {
            (*self as u16).to_value()
        } else {
            None
        }
    }
}

impl Encodable for u16 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
        // Short can only be 12 bits
        if *self > 2047 {
            return u64::from(*self).write_fleece_to(writer, is_wide);
        }
        let val = self.to_value()?;
        if is_wide {
            writer.write_all(&val.bytes).ok()?; Some(4)
        } else {
            writer.write_all(&val.bytes[0..2]).ok()?; Some(2)
        }
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_value(&self) -> Option<SizedValue> {
        if *self > 2047 {
            return None;
        }
        let mut bytes = self.to_be_bytes();
        bytes[0] |= value::tag::SHORT;
        Some(SizedValue::from_narrow(bytes))
    }
}

impl Encodable for i16 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
        // Short can only be 12 bits
        if *self > 2047 || *self < -2048 {
            return i64::from(*self).write_fleece_to(writer, is_wide);
        }
        let val = self.to_value()?;
        if is_wide {
            writer.write_all(&val.bytes).ok()?; Some(4)
        } else {
            writer.write_all(&val.bytes[0..2]).ok()?; Some(2)
        }
    }

    fn fleece_size(&self) -> usize {
        2
    }
    fn to_value(&self) -> Option<SizedValue> {
        if *self > 2047 || *self < -2048 {
            return None;
        }
        let mut bytes = self.to_be_bytes();
        // Make sure to zero out the top 4 bits (where the tag goes) in-case of sign extension
        bytes[0] = (bytes[0] & 0x0F) | value::tag::SHORT;
        Some(SizedValue::from_narrow(bytes))
    }

}

impl Encodable for f32 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, _is_wide: bool) -> Option<usize> {
        let mut buf = [0_u8; 6];
        buf[0] = value::tag::FLOAT;
        buf[2..6].copy_from_slice(&self.to_le_bytes());
        writer.write_all(&buf).ok()?;
        Some(6)
    }

    fn fleece_size(&self) -> usize {
        6
    }

    fn to_value(&self) -> Option<SizedValue> {
        None
    }

}

impl Encodable for f64 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, _is_wide: bool) -> Option<usize> {
        let mut buf = [0_u8; 10];
        buf[0] = value::tag::FLOAT | 0x08;
        buf[2..10].copy_from_slice(&self.to_le_bytes());
        writer.write_all(&buf).ok()?;
        Some(10)
    }

    fn fleece_size(&self) -> usize {
        10
    }

    fn to_value(&self) -> Option<SizedValue> {
        None
    }

}

impl Encodable for bool {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
        let val = self.to_value()?;
        if is_wide {
            writer.write_all(&val.bytes).ok()?;
            Some(4)
        } else {
            writer.write_all(&val.bytes[0..2]).ok()?;
            Some(2)
        }
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_value(&self) -> Option<SizedValue> {
        if *self {
            Some(SizedValue::from_narrow(value::constants::TRUE))
        } else {
            Some(SizedValue::from_narrow(value::constants::FALSE))
        }
    }

}

impl Encodable for NullValue {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
        let val = self.to_value()?;
        if is_wide {
            writer.write_all(&val.bytes).ok()?;
            Some(4)
        } else {
            writer.write_all(&val.bytes[0..2]).ok()?;
            Some(2)
        }
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_value(&self) -> Option<SizedValue> {
        Some(SizedValue::from_narrow(value::constants::NULL))
    }

}

impl Encodable for UndefinedValue {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
        let val = self.to_value()?;
        if is_wide {
            writer.write_all(&val.bytes).ok()?;
            Some(4)
        } else {
            writer.write_all(&val.bytes[0..2]).ok()?;
            Some(2)
        }
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_value(&self) -> Option<SizedValue> {
        Some(SizedValue::from_narrow(value::constants::UNDEFINED))
    }

}

// Data and String are encoded the same in Fleece, apart from the value type tag.
fn write_fleece_string<W: Write, const IS_STR: bool>(string: &[u8], writer: &mut W, is_wide: bool) -> Option<usize> {
    let mut buf = [0_u8; 4];
    buf[0] = if IS_STR {
        value::tag::STRING
    } else {
        value::tag::DATA
    };

    match string.len() {
        // If size is 1 or 0, size fits in the tiny value and string fits in the second byte.
        0 => if is_wide { writer.write_all(&buf).ok()?; Some(4) } else { writer.write_all(&buf[..2]).ok()?; Some(2) }
        1 => {
            buf[0] |= 1;
            buf[1] = string[0];
            if is_wide { writer.write_all(&buf).ok()?; Some(4) } else { writer.write_all(&buf[..2]).ok()?; Some(2) }
        }
        // If size is up to 0x0E (0x0F is the bit that indicates a varint), we can fit the size in the tiny value.
        #[allow(clippy::cast_possible_truncation)]
        2..=0x0E => {
            buf[0] |= string.len() as u8;
            writer.write_all(&buf[0..1]).ok()?;
            writer.write_all(string).ok()?;
            Some(string.len() + 1)
        }
        // Any larger sizes will store the size as a varint.
        _ => {
            buf[0] |= 0x0F;
            let mut varint_buf = [0_u8; varint::MAX_LEN];
            let varint_size = varint::write(&mut varint_buf, string.len() as u64);
            // Write the tag + tiny (1 byte)
            writer.write_all(&buf[0..1]).ok()?;
            // Write the varint
            writer.write_all(&varint_buf[..varint_size]).ok()?;
            // Write the string
            writer.write_all(string).ok()?;
            Some(1 + varint_size + string.len())
        }
    }
}

impl Encodable for [u8] {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
        write_fleece_string::<_, false>(self, writer, is_wide)
    }

    fn fleece_size(&self) -> usize {
        match self.len() {
            0 | 1 => 2,
            2..=0x0E => 1 + self.len(),
            _ => 1 + varint::size_required(self.len()) + self.len(),
        }
    }

    fn to_value(&self) -> Option<SizedValue> {
        match self.len() {
            0 => Some(SizedValue{ bytes: [value::tag::DATA, 0, 0, 0] }),
            1 => Some(SizedValue{ bytes: [value::tag::DATA | 0x01, self[0], 0, 0] }),
            _ => None
        }
    }
}

impl Encodable for str {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
        write_fleece_string::<_, true>(self.as_bytes(), writer, is_wide)
    }

    fn fleece_size(&self) -> usize {
        self.as_bytes().fleece_size()
    }

    fn to_value(&self) -> Option<SizedValue> {
        match self.len() {
            0 => Some(SizedValue{ bytes: [value::tag::STRING, 0, 0, 0] }),
            1 => Some(SizedValue{ bytes: [value::tag::STRING | 0x01, self.as_bytes()[0], 0, 0] }),
            _ => None
        }
    }

}

impl<T> Encodable for Option<T>
where
    T: Encodable,
{
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
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
    fn to_value(&self) -> Option<SizedValue> {
        match self {
            Some(v) => v.to_value(),
            None => NullValue.to_value(),
        }
    }
}

impl Encodable for SizedValue {
    fn write_fleece_to<W: Write>(&self, writer: &mut W, is_wide: bool) -> Option<usize> {
        if is_wide {
            writer.write_all(&self.bytes).ok()?;
            Some(4)
        } else {
            writer.write_all(&self.bytes[0..2]).ok()?;
            Some(2)
        }
    }

    fn fleece_size(&self) -> usize {
        if self.is_wide() {
            4
        } else {
            2
        }
    }

    fn to_value(&self) -> Option<SizedValue> {
        Some(self.clone())
    }
}