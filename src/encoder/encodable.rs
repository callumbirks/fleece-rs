use core::num::NonZeroUsize;

use crate::encoder::value_stack;
use crate::encoder::{Encodable, NullValue, UndefinedValue};
use crate::value::{array, varint};
use crate::value::{pointer, SizedValue};
use crate::{value, ValueType};

// All the built-in implementations of [`Encodable`].

impl<T: super::private::Sealed + ?Sized> super::private::Sealed for &T {}
impl<T: Encodable + ?Sized> Encodable for &T {
    #[inline]
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        (*self).write_fleece_to(buf, is_wide)
    }

    #[inline]
    fn fleece_size(&self) -> usize {
        (*self).fleece_size()
    }

    #[inline]
    fn to_sized_value(&self) -> Option<SizedValue> {
        (*self).to_sized_value()
    }
}

impl super::private::Sealed for i64 {}
impl Encodable for i64 {
    #[allow(clippy::cast_possible_truncation)]
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        // If this is small enough, store it as a short
        if *self <= 2047 || *self >= -2048 {
            #[allow(clippy::cast_possible_truncation)]
            return (*self as i16).write_fleece_to(buf, is_wide);
        }
        let fleece_size = self.fleece_size();
        if fleece_size > buf.len() {
            return None;
        }
        let byte_count = fleece_size - 1;
        buf[0] = value::tag::INT | ((byte_count as u8) - 1);
        buf[1..=byte_count].copy_from_slice(&self.to_le_bytes()[..byte_count]);
        unsafe { Some(NonZeroUsize::new_unchecked(byte_count + 1)) }
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

impl super::private::Sealed for u64 {}
impl Encodable for u64 {
    #[allow(clippy::cast_possible_truncation)] // Suppress warning for `byte_count as u8`
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        // If this is small enough, store it as a short
        if *self <= 2047 {
            #[allow(clippy::cast_possible_truncation)]
            return (*self as u16).write_fleece_to(buf, is_wide);
        }
        let fleece_size = self.fleece_size();
        if fleece_size > buf.len() {
            return None;
        }
        let byte_count = fleece_size - 1;
        buf[0] = value::tag::INT | value::extra_flags::UNSIGNED_INT | ((byte_count as u8) - 1);
        buf[1..=byte_count].copy_from_slice(&self.to_le_bytes()[..byte_count]);
        unsafe { Some(NonZeroUsize::new_unchecked(byte_count + 1)) }
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

impl super::private::Sealed for i32 {}
impl Encodable for i32 {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        i64::from(*self).write_fleece_to(buf, is_wide)
    }

    fn fleece_size(&self) -> usize {
        i64::from(*self).fleece_size()
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        i64::from(*self).to_sized_value()
    }
}

impl super::private::Sealed for u32 {}
impl Encodable for u32 {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        u64::from(*self).write_fleece_to(buf, is_wide)
    }

    fn fleece_size(&self) -> usize {
        u64::from(*self).fleece_size()
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        u64::from(*self).to_sized_value()
    }
}

impl super::private::Sealed for u16 {}
impl Encodable for u16 {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        // Short can only be 12 bits
        if *self > 2047 {
            return u64::from(*self).write_fleece_to(buf, is_wide);
        }
        let Some(val) = self.to_sized_value() else {
            unreachable!();
        };
        val.write_fleece_to(buf, is_wide)
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
        Some(SizedValue::new_narrow(bytes))
    }
}

impl super::private::Sealed for i16 {}
impl Encodable for i16 {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        // Short can only be 12 bits
        if *self > 2047 || *self < -2048 {
            return i64::from(*self).write_fleece_to(buf, is_wide);
        }
        let Some(val) = self.to_sized_value() else {
            unreachable!();
        };
        val.write_fleece_to(buf, is_wide)
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
        Some(SizedValue::new_narrow(bytes))
    }
}

impl super::private::Sealed for u8 {}
impl Encodable for u8 {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        (u16::from(*self)).write_fleece_to(buf, is_wide)
    }

    fn fleece_size(&self) -> usize {
        (u16::from(*self)).fleece_size()
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        (u16::from(*self)).to_sized_value()
    }
}

impl super::private::Sealed for i8 {}
impl Encodable for i8 {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        (i16::from(*self)).write_fleece_to(buf, is_wide)
    }

    fn fleece_size(&self) -> usize {
        (i16::from(*self)).fleece_size()
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        (i16::from(*self)).to_sized_value()
    }
}

impl super::private::Sealed for f32 {}
impl Encodable for f32 {
    fn write_fleece_to(&self, buf: &mut [u8], _is_wide: bool) -> Option<NonZeroUsize> {
        if self.fleece_size() > buf.len() {
            return None;
        }
        buf[0] = value::tag::FLOAT;
        buf[1] = 0;
        buf[2..6].copy_from_slice(&self.to_le_bytes());
        unsafe { Some(NonZeroUsize::new_unchecked(6)) }
    }

    fn fleece_size(&self) -> usize {
        6
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        None
    }
}

impl super::private::Sealed for f64 {}
impl Encodable for f64 {
    fn write_fleece_to(&self, buf: &mut [u8], _is_wide: bool) -> Option<NonZeroUsize> {
        if self.fleece_size() > buf.len() {
            return None;
        }

        buf[0] = value::tag::FLOAT | 0x08;
        buf[1] = 0;
        buf[2..10].copy_from_slice(&self.to_le_bytes());
        unsafe { Some(NonZeroUsize::new_unchecked(10)) }
    }

    fn fleece_size(&self) -> usize {
        10
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        None
    }
}

fn write_fleece_constant(buf: &mut [u8], constant: [u8; 2], is_wide: bool) -> Option<NonZeroUsize> {
    if is_wide {
        if buf.len() < 4 {
            return None;
        }
        buf[0..2].copy_from_slice(&constant);
        buf[2] = 0;
        buf[3] = 0;
        unsafe { Some(NonZeroUsize::new_unchecked(4)) }
    } else {
        if buf.len() < 2 {
            return None;
        }
        buf[0..2].copy_from_slice(&constant);
        unsafe { Some(NonZeroUsize::new_unchecked(2)) }
    }
}

impl super::private::Sealed for bool {}
impl Encodable for bool {
    fn write_fleece_to(&self, vec: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        if *self {
            write_fleece_constant(vec, value::constants::TRUE, is_wide)
        } else {
            write_fleece_constant(vec, value::constants::FALSE, is_wide)
        }
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        if *self {
            Some(SizedValue::new_narrow(value::constants::TRUE))
        } else {
            Some(SizedValue::new_narrow(value::constants::FALSE))
        }
    }
}

impl super::private::Sealed for NullValue {}
impl Encodable for NullValue {
    fn write_fleece_to(&self, vec: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        write_fleece_constant(vec, value::constants::NULL, is_wide)
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        Some(SizedValue::new_narrow(value::constants::NULL))
    }
}

impl super::private::Sealed for UndefinedValue {}
impl Encodable for UndefinedValue {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        write_fleece_constant(buf, value::constants::UNDEFINED, is_wide)
    }

    fn fleece_size(&self) -> usize {
        2
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        Some(SizedValue::new_narrow(value::constants::UNDEFINED))
    }
}

// Data and String are encoded the same in Fleece, apart from the value type tag.
// INVARIANTS:
//   - `buf` MUST be large enough to encode `string`.
fn write_fleece_string<const IS_STR: bool>(
    string: &[u8],
    buf: &mut [u8],
    is_wide: bool,
) -> NonZeroUsize {
    let tag = if IS_STR {
        value::tag::STRING
    } else {
        value::tag::DATA
    };

    match string.len() {
        // If size is 1 or 0, size fits in the tiny value and string fits in the second byte.
        0 => {
            if is_wide {
                buf[0] = tag;
                buf[1..4].copy_from_slice(&[0, 0, 0]);
                unsafe { NonZeroUsize::new_unchecked(4) }
            } else {
                buf[0] = tag;
                buf[1] = 0;
                unsafe { NonZeroUsize::new_unchecked(2) }
            }
        }
        1 => {
            if is_wide {
                buf[0] = tag | 1;
                buf[1] = string[0];
                buf[2..4].copy_from_slice(&[0, 0]);
                unsafe { NonZeroUsize::new_unchecked(4) }
            } else {
                buf[0] = tag | 1;
                buf[1] = string[0];
                unsafe { NonZeroUsize::new_unchecked(2) }
            }
        }
        // If size is up to 0x0E (0x0F is the bit that indicates a varint), we can fit the size in the tiny value.
        #[allow(clippy::cast_possible_truncation)]
        2..=0x0E => {
            let len = string.len();
            buf[0] = tag | len as u8;
            buf[1..=len].copy_from_slice(string);
            unsafe { NonZeroUsize::new_unchecked(len + 1) }
        }
        // Any larger sizes will store the size as a varint.
        len => {
            buf[0] = tag | 0x0F;
            let mut varint_buf = [0_u8; varint::MAX_LEN];
            let varint_size = varint::write(&mut varint_buf, string.len() as u64);
            // Write the varint
            buf[1..=varint_size].copy_from_slice(&varint_buf[..varint_size]);
            // Write the string
            buf[(1 + varint_size)..(1 + varint_size + len)].copy_from_slice(string);
            unsafe { NonZeroUsize::new_unchecked(1 + varint_size + len) }
        }
    }
}

impl super::private::Sealed for [u8] {}
impl Encodable for [u8] {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        if self.fleece_size() > buf.len() {
            None
        } else {
            Some(write_fleece_string::<false>(self, buf, is_wide))
        }
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
            0 => Some(SizedValue::new_narrow([value::tag::DATA, 0])),
            1 => Some(SizedValue::new_narrow([value::tag::DATA | 0x01, self[0]])),
            _ => None,
        }
    }
}

impl super::private::Sealed for str {}
impl Encodable for str {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        if self.fleece_size() > buf.len() {
            None
        } else {
            Some(write_fleece_string::<true>(self.as_bytes(), buf, is_wide))
        }
    }

    fn fleece_size(&self) -> usize {
        self.as_bytes().fleece_size()
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        match self.len() {
            0 => Some(SizedValue::new_narrow([value::tag::STRING, 0])),
            1 => Some(SizedValue::new_narrow([
                value::tag::STRING | 0x01,
                self.as_bytes()[0],
            ])),
            _ => None,
        }
    }
}

impl<T> super::private::Sealed for Option<T> {}
impl<T> Encodable for Option<T>
where
    T: Encodable,
{
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        match self {
            Some(value) => value.write_fleece_to(buf, is_wide),
            None => NullValue.write_fleece_to(buf, is_wide),
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

impl super::private::Sealed for SizedValue {}
impl Encodable for SizedValue {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        if self.value_type() == ValueType::Pointer {
            let offset = self.pointer_offset();

            if offset > u32::from(pointer::MAX_NARROW) || is_wide {
                if buf.len() < 4 {
                    return None;
                }
                buf[0..4].copy_from_slice(&(offset >> 1).to_be_bytes());
                buf[0] |= value::tag::POINTER;
                unsafe { Some(NonZeroUsize::new_unchecked(4)) }
            } else {
                if buf.len() < 2 {
                    return None;
                }

                #[cfg(debug_assertions)]
                let offset =
                    u16::try_from(offset).expect("offset should be <= pointer::MAX_NARROW");
                #[cfg(not(debug_assertions))]
                #[allow(clippy::cast_possible_truncation)]
                let offset = offset as u16;

                buf[0..2].copy_from_slice(&(offset >> 1).to_be_bytes());
                buf[0] |= value::tag::POINTER;
                unsafe { Some(NonZeroUsize::new_unchecked(2)) }
            }
        } else if is_wide {
            if buf.len() < 4 {
                return None;
            }
            buf[0..4].copy_from_slice(self.as_bytes());
            unsafe { Some(NonZeroUsize::new_unchecked(4)) }
        } else {
            if buf.len() < 2 {
                return None;
            }
            buf[0..2].copy_from_slice(&self.as_bytes()[..2]);
            buf[0] &= 0xBF;
            unsafe { Some(NonZeroUsize::new_unchecked(2)) }
        }
    }

    fn fleece_size(&self) -> usize {
        if self.value_type() == ValueType::Pointer {
            if self.pointer_offset() > u32::from(pointer::MAX_NARROW) {
                4
            } else {
                2
            }
        } else {
            2
        }
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        Some(*self)
    }
}

// INVARIANTS:
//   - `buf` MUST be large enough to encode `string`.
fn write_valuestack_collection(buf: &mut [u8], tag: u8, len: usize, is_wide: bool) -> NonZeroUsize {
    #[allow(clippy::cast_possible_truncation)]
    let inline_size = len.min(array::VARINT_COUNT as usize) as u16;

    buf[0] = if is_wide {
        0x08 | tag | (inline_size >> 8) as u8
    } else {
        tag | (inline_size >> 8) as u8
    };
    buf[1] = (inline_size & 0xFF) as u8;

    let written = if len >= array::VARINT_COUNT as usize {
        let mut varint_buf = [0_u8; varint::MAX_LEN];
        let varint_size = varint::write(&mut varint_buf, len as u64);

        buf[2..(2 + varint_size)].copy_from_slice(&varint_buf[..varint_size]);
        2 + varint_size
    } else {
        2
    };

    unsafe { NonZeroUsize::new_unchecked(written) }
}

// Just write the Array header, not the values
impl super::private::Sealed for value_stack::Array {}
impl Encodable for value_stack::Array {
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        if self.fleece_size() > buf.len() {
            None
        } else {
            Some(write_valuestack_collection(
                buf,
                value::tag::ARRAY,
                self.values.len(),
                is_wide,
            ))
        }
    }

    fn fleece_size(&self) -> usize {
        let len = self.values.len();
        2 + if len >= array::VARINT_COUNT as usize {
            varint::size_required(len as u64)
        } else {
            0
        }
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        None
    }
}

impl super::private::Sealed for value_stack::Dict {}
impl Encodable for value_stack::Dict {
    // Just write the Dict header, not the values
    fn write_fleece_to(&self, buf: &mut [u8], is_wide: bool) -> Option<NonZeroUsize> {
        if self.fleece_size() > buf.len() {
            None
        } else {
            Some(write_valuestack_collection(
                buf,
                value::tag::DICT,
                self.values.len(),
                is_wide,
            ))
        }
    }

    fn fleece_size(&self) -> usize {
        let len = self.values.len();
        2 + if len >= array::VARINT_COUNT as usize {
            varint::size_required(len as u64)
        } else {
            0
        }
    }

    fn to_sized_value(&self) -> Option<SizedValue> {
        None
    }
}
