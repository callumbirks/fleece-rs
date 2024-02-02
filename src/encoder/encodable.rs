use crate::encoder::{Encodable, NullValue, UndefinedValue};
use crate::raw::{value, varint};

impl Encodable for i64 {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        // If this is small enough, store it as a short
        if *self <= 2047 || *self >= -2048 {
            #[allow(clippy::cast_possible_truncation)]
            return (*self as i16).write_fleece_to(bytes);
        }
        let mut buf = [0_u8; 9];
        buf[1..].copy_from_slice(&self.to_le_bytes());
        #[allow(clippy::cast_possible_truncation)]
        let trailing_zeros = buf.iter().rev().take_while(|b| **b == 0).count() as u8;
        let size = 8 - trailing_zeros;
        if bytes.len() < size as usize + 1 {
            return None;
        }
        buf[0] = value::tag::INT | (size - 1);
        bytes[..=(size as usize)].copy_from_slice(&buf[..=(size as usize)]);
        Some(())
    }
}

impl Encodable for u64 {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        // If this is small enough, store it as a short
        if *self <= 2047 {
            #[allow(clippy::cast_possible_truncation)]
            return (*self as u16).write_fleece_to(bytes);
        }
        let mut buf = [0_u8; 9];
        buf[1..].copy_from_slice(&self.to_le_bytes());
        #[allow(clippy::cast_possible_truncation)]
        let trailing_zeros = buf.iter().rev().take_while(|b| **b == 0).count() as u8;
        let size = 8 - trailing_zeros;
        if bytes.len() < size as usize + 1 {
            return None;
        }
        buf[0] = value::tag::INT | 0x08 | ((size - 1) & 0x07);
        bytes[..=(size as usize)].copy_from_slice(&buf[..=(size as usize)]);
        Some(())
    }
}

impl Encodable for u16 {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        if bytes.len() < 2 {
            return None;
        }
        // Short can only be 12 bits
        if *self > 2047 {
            return u64::from(*self).write_fleece_to(bytes);
        }
        bytes[0..2].copy_from_slice(&self.to_be_bytes());
        bytes[0] |= value::tag::SHORT;
        Some(())
    }
}

impl Encodable for i16 {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        if bytes.len() < 2 {
            return None;
        }
        // Short can only be 12 bits
        if *self > 2047 || *self < -2048 {
            return i64::from(*self).write_fleece_to(bytes);
        }
        bytes[0..2].copy_from_slice(&self.to_be_bytes());
        bytes[0] = (bytes[0] & 0x0F) | value::tag::SHORT;
        Some(())
    }
}

impl Encodable for f32 {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        if bytes.len() < 6 {
            return None;
        }
        bytes[0] = value::tag::FLOAT;
        bytes[2..6].copy_from_slice(&self.to_le_bytes());
        Some(())
    }
}

impl Encodable for f64 {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        if bytes.len() < 10 {
            return None;
        }
        bytes[0] = value::tag::FLOAT | 0x08;
        bytes[2..10].copy_from_slice(&self.to_le_bytes());
        Some(())
    }
}

fn write_fleece_constant(bytes: &mut [u8], constant: [u8; 2]) -> Option<()> {
    if bytes.len() < 2 {
        return None;
    }
    bytes[0..2].copy_from_slice(&constant);
    Some(())
}

impl Encodable for bool {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        if *self {
            write_fleece_constant(bytes, value::constants::TRUE)
        } else {
            write_fleece_constant(bytes, value::constants::FALSE)
        }
    }
}

impl Encodable for NullValue {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        write_fleece_constant(bytes, value::constants::NULL)
    }
}

impl Encodable for UndefinedValue {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        write_fleece_constant(bytes, value::constants::UNDEFINED)
    }
}

// Data and String are encoded the same in Fleece, apart from the value type tag.
fn write_fleece_string<const IS_STR: bool>(string: &[u8], bytes: &mut [u8]) -> Option<()> {
    if bytes.len() < 2 {
        return None;
    }

    bytes[0] = if IS_STR {
        value::tag::STRING
    } else {
        value::tag::DATA
    };

    match string.len() {
        // If size is 1 or 0, size fits in the tiny value and string fits in the second byte.
        0 => {}
        1 => {
            bytes[0] |= 1;
            bytes[1] = string[0];
        }
        // If size is up to 0x0E (0x0F is the bit that indicates a varint), we can fit the size in the tiny value.
        #[allow(clippy::cast_possible_truncation)]
        2..=0x0E => {
            if bytes.len() < 1 + string.len() {
                return None;
            }
            bytes[0] |= string.len() as u8;
            bytes[1..=string.len()].copy_from_slice(string);
        }
        // Any larger sizes will store the size as a varint.
        _ => {
            bytes[0] |= 0x0F;
            let mut varint_buf = [0_u8; varint::MAX_LEN];
            let varint_size = varint::write(&mut varint_buf, string.len() as u64);
            let total_size = 1 + varint_size + string.len();
            if bytes.len() < total_size {
                return None;
            }
            bytes[1..=varint_size].copy_from_slice(&varint_buf[..varint_size]);
            bytes[varint_size + 1..total_size].copy_from_slice(string);
        }
    }
    Some(())
}

impl Encodable for str {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        write_fleece_string::<true>(self.as_bytes(), bytes)
    }
}

impl Encodable for [u8] {
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        write_fleece_string::<false>(self, bytes)
    }
}

impl<T> Encodable for Option<T>
where
    T: Encodable,
{
    fn write_fleece_to(&self, bytes: &mut [u8]) -> Option<()> {
        match self {
            Some(value) => value.write_fleece_to(bytes),
            None => NullValue.write_fleece_to(bytes),
        }
    }
}
