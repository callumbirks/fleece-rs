use std::io::{self, Write};
use crate::raw::value;

struct Encoder<W> where W : Write {
    writer: io::BufWriter<W>,
}

trait Encodable {
    fn encode(&self, encoder: &mut Encoder<impl Write>);
}

impl Encodable for i64 {
    fn encode(&self, encoder: &mut Encoder<impl Write>) {
        let mut buf = [0_u8; 9];
        buf[1..].copy_from_slice(&self.to_le_bytes());
        #[allow(clippy::cast_possible_truncation)]
        let trailing_zeros = buf.iter().rev().take_while(|b| **b == 0).count() as u8;
        let size = 8 - trailing_zeros;
        buf[0] = value::tag::INT | (size - 1);
        encoder.write_bytes(&buf[..=(size as usize)]).expect("Failed to write i64 bytes");
    }
}

impl Encodable for u64 {
    fn encode(&self, encoder: &mut Encoder<impl Write>) {
        let mut buf = [0_u8; 9];
        buf[1..].copy_from_slice(&self.to_le_bytes());
        #[allow(clippy::cast_possible_truncation)]
        let trailing_zeros = buf.iter().rev().take_while(|b| **b == 0).count() as u8;
        let size = 8 - trailing_zeros;
        buf[0] = value::tag::INT | 0x08 | ((size - 1) & 0x07);
        encoder.write_bytes(&buf[..=(size as usize)]).expect("Failed to write u64 bytes");
    }
}

impl Encodable for f32 {
    fn encode(&self, encoder: &mut Encoder<impl Write>) {
        let mut buf = [0_u8; 6];
        buf[0] = value::tag::FLOAT;
        buf[2..].copy_from_slice(&self.to_le_bytes());
        encoder.write_bytes(&buf).expect("Failed to write f32 bytes");
    }
}

impl Encodable for f64 {
    fn encode(&self, encoder: &mut Encoder<impl Write>) {
        let mut buf = [0_u8; 10];
        buf[0] = value::tag::FLOAT | 0x08;
        buf[2..].copy_from_slice(&self.to_le_bytes());
        encoder.write_bytes(&buf).expect("Failed to write f64 bytes");
    }
}

impl Encodable for bool {
    fn encode(&self, encoder: &mut Encoder<impl Write>) {
        if *self {
            encoder.write_bytes(&value::constants::TRUE).expect("Failed to write bool bytes");
        } else {
            encoder.write_bytes(&value::constants::FALSE).expect("Failed to write bool bytes");
        }
    }
}

impl<W: Write> Encoder<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: io::BufWriter::new(writer),
        }
    }

    fn write<T>(&mut self, value: &T) where T: Encodable {
        value.encode(self);
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)
    }
}
