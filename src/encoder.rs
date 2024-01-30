use crate::raw::value;
use crate::raw::varint;
use std::io::{self, Write};

struct Encoder<W>
where
    W: Write,
{
    writer: io::BufWriter<W>,
}

struct NullValue;
struct UndefinedValue;

trait Encodable {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()>;
}

impl Encodable for i64 {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        // If this is small enough, store it as a short
        if *self <= 2047 || *self >= -2048 {
            return (*self as i16).encode(encoder);
        }
        let mut buf = [0_u8; 9];
        buf[1..].copy_from_slice(&self.to_le_bytes());
        #[allow(clippy::cast_possible_truncation)]
        let trailing_zeros = buf.iter().rev().take_while(|b| **b == 0).count() as u8;
        let size = 8 - trailing_zeros;
        buf[0] = value::tag::INT | (size - 1);
        encoder.write_bytes(&buf[..=(size as usize)])
    }
}

impl Encodable for u64 {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        // If this is small enough, store it as a short
        if *self <= 2047 {
            return (*self as u16).encode(encoder);
        }
        let mut buf = [0_u8; 9];
        buf[1..].copy_from_slice(&self.to_le_bytes());
        #[allow(clippy::cast_possible_truncation)]
        let trailing_zeros = buf.iter().rev().take_while(|b| **b == 0).count() as u8;
        let size = 8 - trailing_zeros;
        buf[0] = value::tag::INT | 0x08 | ((size - 1) & 0x07);
        encoder.write_bytes(&buf[..=(size as usize)])
    }
}

impl Encodable for u16 {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        // Short can only be 12 bits
        if *self > 2047 {
            return u64::from(*self).encode(encoder);
        }
        let mut buf: [u8; 2] = self.to_be_bytes();
        buf[0] |= value::tag::SHORT;
        encoder.write_bytes(&buf)
    }
}

impl Encodable for i16 {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        // Short can only be 12 bits
        if *self > 2047 || *self < -2048 {
            return i64::from(*self).encode(encoder);
        }
        let mut buf: [u8; 2] = self.to_be_bytes();
        // Signed ints will be sign extended, we can just zero out the top 4 bits
        buf[0] = buf[0] & 0x0F | value::tag::SHORT;
        encoder.write_bytes(&buf)
    }
}

impl Encodable for f32 {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        let mut buf = [0_u8; 6];
        buf[0] = value::tag::FLOAT;
        buf[2..].copy_from_slice(&self.to_le_bytes());
        encoder.write_bytes(&buf)
    }
}

impl Encodable for f64 {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        let mut buf = [0_u8; 10];
        buf[0] = value::tag::FLOAT | 0x08;
        buf[2..].copy_from_slice(&self.to_le_bytes());
        encoder.write_bytes(&buf)
    }
}

impl Encodable for bool {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        if *self {
            encoder.write_bytes(&value::constants::TRUE)
        } else {
            encoder.write_bytes(&value::constants::FALSE)
        }
    }
}

impl Encodable for NullValue {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        encoder.write_bytes(&value::constants::NULL)
    }
}

impl Encodable for UndefinedValue {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        encoder.write_bytes(&value::constants::UNDEFINED)
    }
}

// Data and String are the same, except for the tag. So we use this trait in order to use the same method for both
trait ByteEncode {
    fn encode_data<const IS_STR: bool>(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()>;
}

impl ByteEncode for [u8] {
    fn encode_data<const IS_STR: bool>(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        let mut buf = if IS_STR {
            vec![value::tag::STRING, 0]
        } else {
            vec![value::tag::DATA, 0]
        };
        match self.len() {
            0 => {}
            1 => {
                buf[0] |= 1;
                buf[1] = self[0];
            }
            #[allow(clippy::cast_possible_truncation)]
            2..=0x0E => {
                buf[0] |= self.len() as u8;
                buf.resize(1 + self.len(), 0);
                buf[1..].copy_from_slice(self);
            }
            _ => {
                buf[0] |= 0x0F;
                buf.resize(1 + varint::MAX_LEN + self.len(), 0);
                let written = varint::write(&mut buf[1..=varint::MAX_LEN], self.len() as u64);
                buf.resize(1 + written + self.len(), 0);
                buf[written + 1..].copy_from_slice(self);
            }
        }
        encoder.write_bytes(&buf)
    }
}

impl Encodable for str {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        self.as_bytes().encode_data::<true>(encoder)
    }
}

impl Encodable for [u8] {
    fn encode(&self, encoder: &mut Encoder<impl Write>) -> io::Result<()> {
        self.encode_data::<false>(encoder)
    }
}

impl<W: Write> Encoder<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: io::BufWriter::new(writer),
        }
    }

    pub fn write<T>(&mut self, value: &T) -> io::Result<()>
    where
        T: Encodable,
    {
        value.encode(self)
    }

    pub fn begin_dict(&mut self) -> io::Result<()> {
        todo!()
    }

    pub fn end_dict(&mut self) -> io::Result<()> {
        todo!()
    }

    pub fn begin_array(&mut self) -> io::Result<()> {
        todo!()
    }

    pub fn end_array(&mut self) -> io::Result<()> {
        todo!()
    }

    pub fn finish(&mut self) -> io::Result<()> {
        self.writer.flush()?;
        todo!("Close any open arrays, dicts, etc");
    }

    pub(super) fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)
    }
}
