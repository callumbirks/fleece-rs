use crate::encoder::{Encodable, NullValue, UndefinedValue};
use crate::raw::{value, varint};
use std::io::Write;

fn write_fleece_int<W: Write, const UNSIGNED: bool>(writer: &mut W, value: i64) -> Option<()> {
    let mut buf = [0_u8; 9];
    //#[allow(clippy::cast_possible_truncation)]
    //let trailing_zeros = if UNSIGNED || value >= 0 {
    //    buf.iter().rev().take_while(|b| **b == 0).count() as u8
    //} else {
    //    // Signed integers are sign extended, so we need to count the trailing 1s
    //    buf.iter().rev().take_while(|b| **b == 0xFF).count() as u8
    //};
    //let size = 8 - trailing_zeros;
    let byte_count = value.fleece_size() - 1;
    buf[0] = value::tag::INT | ((byte_count as u8) - 1);
    buf[1..].copy_from_slice(&value.to_le_bytes());
    writer.write_all(&buf[..=(byte_count as usize)]).ok()
}

impl Encodable for i64 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        // If this is small enough, store it as a short
        if *self <= 2047 || *self >= -2048 {
            #[allow(clippy::cast_possible_truncation)]
            return (*self as i16).write_fleece_to(writer);
        }
        return write_fleece_int::<_, false>(writer, *self);
    }

    fn fleece_size(&self) -> usize {
        if *self <= 2047 || *self >= -2048 {
            return 2;
        }
        return if *self >= 0 {
            8 - self.trailing_zeros() + 1
        } else {
            8 - self.trailing_ones() + 1
        } as usize;
    }
}

impl Encodable for u64 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        // If this is small enough, store it as a short
        if *self <= 2047 {
            #[allow(clippy::cast_possible_truncation)]
            return (*self as u16).write_fleece_to(writer);
        }
        return write_fleece_int::<_, true>(writer, *self as i64);
    }

    fn fleece_size(&self) -> usize {
        if *self <= 2047 {
            2
        } else {
            8 - self.trailing_zeros() as usize + 1
        }
    }
}

impl Encodable for u16 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        // Short can only be 12 bits
        if *self > 2047 {
            return u64::from(*self).write_fleece_to(writer);
        }
        let mut buf: [u8; 2] = self.to_be_bytes();
        buf[0] |= value::tag::SHORT;
        writer.write_all(&buf).ok()
    }

    fn fleece_size(&self) -> usize {
        2
    }
}

impl Encodable for i16 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        // Short can only be 12 bits
        if *self > 2047 || *self < -2048 {
            return i64::from(*self).write_fleece_to(writer);
        }
        let mut buf: [u8; 2] = self.to_be_bytes();
        // Make sure to zero out the top 4 bits (where the tag goes) in-case of sign extension
        buf[0] = (buf[0] & 0x0F) | value::tag::SHORT;
        writer.write_all(&buf).ok()
    }

    fn fleece_size(&self) -> usize {
        2
    }
}

impl Encodable for f32 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        let mut buf = [0_u8; 6];
        buf[0] = value::tag::FLOAT;
        buf[2..6].copy_from_slice(&self.to_le_bytes());
        writer.write_all(&buf).ok()
    }

    fn fleece_size(&self) -> usize {
        6
    }
}

impl Encodable for f64 {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        let mut buf = [0_u8; 10];
        buf[0] = value::tag::FLOAT | 0x08;
        buf[2..10].copy_from_slice(&self.to_le_bytes());
        writer.write_all(&buf).ok()
    }

    fn fleece_size(&self) -> usize {
        10
    }
}

fn write_fleece_constant<W: Write>(writer: &mut W, constant: [u8; 2]) -> Option<()> {
    writer.write_all(&constant).ok()
}

impl Encodable for bool {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        if *self {
            write_fleece_constant(writer, value::constants::TRUE)
        } else {
            write_fleece_constant(writer, value::constants::FALSE)
        }
    }

    fn fleece_size(&self) -> usize {
        2
    }
}

impl Encodable for NullValue {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        write_fleece_constant(writer, value::constants::NULL)
    }

    fn fleece_size(&self) -> usize {
        2
    }
}

impl Encodable for UndefinedValue {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        write_fleece_constant(writer, value::constants::UNDEFINED)
    }

    fn fleece_size(&self) -> usize {
        2
    }
}

// Data and String are encoded the same in Fleece, apart from the value type tag.
fn write_fleece_string<W: Write, const IS_STR: bool>(string: &[u8], writer: &mut W) -> Option<()> {
    let mut buf = [0_u8; 2];
    buf[0] = if IS_STR {
        value::tag::STRING
    } else {
        value::tag::DATA
    };

    match string.len() {
        // If size is 1 or 0, size fits in the tiny value and string fits in the second byte.
        0 => writer.write_all(&buf[..2]).ok(),
        1 => {
            buf[0] |= 1;
            buf[1] = string[0];
            writer.write_all(&buf[..2]).ok()
        }
        // If size is up to 0x0E (0x0F is the bit that indicates a varint), we can fit the size in the tiny value.
        #[allow(clippy::cast_possible_truncation)]
        2..=0x0E => {
            buf[0] |= string.len() as u8;
            writer.write_all(&buf[0..1]).ok()?;
            writer.write_all(string).ok()
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
            writer.write_all(string).ok()
        }
    }
}

impl Encodable for [u8] {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        write_fleece_string::<_, false>(self, writer)
    }

    fn fleece_size(&self) -> usize {
        match self.len() {
            0 | 1 => 2,
            2..=0x0E => 1 + self.len(),
            _ => 1 + varint::size_required(self.len()) + self.len(),
        }
    }
}

impl Encodable for str {
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        write_fleece_string::<_, true>(self.as_bytes(), writer)
    }

    fn fleece_size(&self) -> usize {
        self.as_bytes().fleece_size()
    }
}

impl<T> Encodable for Option<T>
where
    T: Encodable,
{
    fn write_fleece_to<W: Write>(&self, writer: &mut W) -> Option<()> {
        match self {
            Some(value) => value.write_fleece_to(writer),
            None => NullValue.write_fleece_to(writer),
        }
    }

    fn fleece_size(&self) -> usize {
        match self {
            Some(value) => value.fleece_size(),
            None => 2,
        }
    }
}
