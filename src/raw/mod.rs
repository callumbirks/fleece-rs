use std::fmt::{Display, Formatter};

#[derive(PartialEq, Eq)]
pub enum ValueType {
    Null,
    Undefined,
    False,
    True,
    Short,
    Int,
    UnsignedShort,
    UnsignedInt,
    Float,
    Double,
    String,
    Data,
    Array,
    Dict,
    Pointer,
}

const VARINT_MAX_LEN: usize = 10;

#[repr(C)]
pub struct RawValue {
    bytes: [u8],
}

impl RawValue {
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn from_data(data: &[u8]) -> Option<&Self> {
        let root = Self::find_root(data)?;
        // wide parameter doesn't matter here, as its only used for pointers, and root will never be a pointer
        if root.validate(false, data.as_ptr(), unsafe {
            data.as_ptr().add(data.len())
        }) {
            Some(root)
        } else {
            None
        }
    }

    fn find_root(data: &[u8]) -> Option<&Self> {
        // Data must be at least 2 bytes, and evenly sized
        if data.is_empty() || data.len() % 2 != 0 {
            return None;
        }
        // Root is 2 bytes at the end of the data
        let root = &data[(data.len() - 2)..];
        let data_end = root.as_ptr();
        let root: &RawValue = unsafe { std::mem::transmute(root) };

        if root.value_type() == ValueType::Pointer {
            return root.ptr_deref_untrusted(false, data.as_ptr(), data_end);
        } else if data.len() == 2 {
            return Some(root);
        }
        None
    }

    pub unsafe fn from_data_unchecked(data: &[u8]) -> &Self {
        // Root is 2 bytes at the end of the data
        let root = &data[(data.len() - 2)..];
        let root: &RawValue = std::mem::transmute(root);
        if root.value_type() == ValueType::Pointer {
            return root.ptr_deref::<false>();
        } else if data.len() == 2 {
            return root;
        }
        panic!("Invalid data");
    }

    // This should only be called on Array or Dict
    fn validate_elements(&self, data_start: *const u8, data_end: *const u8) -> bool {
        let is_dict = self.value_type() == ValueType::Dict;
        let width = if self.arr_is_wide() { 4 } else { 2 };
        let wide = width == 4;
        let elem_count = if is_dict {
            self.arr_len() * 2
        } else {
            self.arr_len()
        };

        let first = unsafe { self.as_ptr().add(2) };
        if (first as usize) + (elem_count * width) > (data_end as usize) {
            println!("First + size > data_end");
            return false;
        }

        let mut current = first;
        let mut elem_count = elem_count;
        while elem_count > 0 {
            let next = unsafe { current.add(width) };
            if let Some(current_value) = self.from_raw(current, width) {
                if !current_value.validate(wide, data_start, next) {
                    println!("Current value failed nested validate");
                    return false;
                }
            } else {
                println!("Current value is None");
                return false;
            }

            current = next;
            elem_count -= 1;
        }
        true
    }

    fn validate(&self, wide: bool, data_start: *const u8, data_end: *const u8) -> bool {
        match self.value_type() {
            ValueType::Array | ValueType::Dict => self.validate_elements(data_start, data_end),
            ValueType::Pointer => {
                if let Some(target) = self.ptr_deref_untrusted(wide, data_start, self.as_ptr()) {
                    target.validate(wide, data_start, self.as_ptr())
                } else {
                    false
                }
            }
            _ => {
                true
                //self.as_ptr() as usize + self.required_size() <= data_end as usize
            }
        }
    }

    // The number of bytes required to hold this value
    // For Dict and Array, this does not include the size of inline values, only the header
    pub fn required_size(&self) -> usize {
        match self.value_type() {
            ValueType::Null
            | ValueType::Undefined
            | ValueType::False
            | ValueType::True
            | ValueType::UnsignedShort
            | ValueType::Short => 2,
            ValueType::UnsignedInt | ValueType::Int => 2 + (self.bytes[0] & 0x07) as usize,
            ValueType::Float => 6,
            ValueType::Double => 10,
            ValueType::String | ValueType::Data => {
                let data = self.get_data();
                if let Some(last) = data.last() {
                    last as *const u8 as usize - self.bytes.as_ptr() as usize + 1
                } else {
                    0
                }
            }
            // TODO: This is not correct for MutableArray / MutableDict
            ValueType::Array | ValueType::Dict => 2,
            // Pointers are 2 or 4 bytes, depending on context
            ValueType::Pointer => 2,
        }
    }

    // Will cause a panic if bytes is empty
    pub fn value_type(&self) -> ValueType {
        let byte = self.bytes[0];
        match byte & 0xF0 {
            0x30 => match byte & 0x0F {
                0x00 => ValueType::Null,
                0x0C => ValueType::Undefined,
                0x04 => ValueType::False,
                0x08 => ValueType::True,
                _ => ValueType::Null,
            },
            0x00 => match byte & 0x08 {
                0x00 => ValueType::Short,
                _ => ValueType::UnsignedShort,
            },
            0x10 => match byte & 0x08 {
                0x00 => ValueType::Int,
                _ => ValueType::UnsignedInt,
            },
            0x20 => match byte & 0x08 {
                0x00 => ValueType::Float,
                _ => ValueType::Double,
            },
            0x40 => ValueType::String,
            0x50 => ValueType::Data,
            0x60 => ValueType::Array,
            0x70 => ValueType::Dict,
            // Pointers are 0x80 to 0xF0
            _ => ValueType::Pointer,
        }
    }

    #[inline(always)]
    fn get_short(&self) -> u16 {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(&self.bytes[0..2]);
        u16::from_be_bytes(buf) & 0x0FFF
    }
}

// Conversions
impl RawValue {
    pub fn as_bool(&self) -> bool {
        match self.value_type() {
            ValueType::False => false,
            ValueType::True => true,
            ValueType::Short | ValueType::Int | ValueType::Float | ValueType::Double => {
                self.as_int() != 0
            }
            _ => true,
        }
    }

    pub fn as_int(&self) -> i64 {
        match self.value_type() {
            ValueType::True => 1,
            ValueType::False => 0,
            ValueType::UnsignedShort => self.get_short() as i64,
            ValueType::Short => {
                let i: u16 = self.get_short();
                if i & 0x0800 != 0 {
                    (i as i16 | 0xF000_u16 as i16) as i64
                } else {
                    i as i64
                }
            }
            ValueType::Int | ValueType::UnsignedInt => {
                let count = (self.bytes[0] & 0x07) as usize + 1;
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&self.bytes[1..count]);
                i64::from_le_bytes(buf)
            }
            ValueType::Float | ValueType::Double => self.as_double() as i64,
            _ => 0,
        }
    }

    pub fn as_unsigned_int(&self) -> u64 {
        self.as_int() as u64
    }

    pub fn as_short(&self) -> i16 {
        match self.value_type() {
            ValueType::True => 1,
            ValueType::False => 0,
            ValueType::UnsignedShort => self.get_short() as i16,
            ValueType::Short => {
                let i: u16 = self.get_short();
                i as i16 | 0xF000_u16 as i16
            }
            ValueType::Int | ValueType::UnsignedInt => self.as_int() as i16,
            ValueType::Float | ValueType::Double => self.as_double() as i16,
            _ => 0,
        }
    }

    pub fn as_unsigned_short(&self) -> u16 {
        self.as_short() as u16
    }

    pub fn as_double(&self) -> f64 {
        match self.value_type() {
            ValueType::Float => {
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&self.bytes[2..6]);
                f32::from_le_bytes(buf) as f64
            }
            ValueType::Double => {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&self.bytes[2..10]);
                f64::from_le_bytes(buf)
            }
            _ => self.as_int() as f64,
        }
    }

    pub fn as_float(&self) -> f32 {
        self.as_double() as f32
    }

    pub fn as_data(&self) -> &[u8] {
        match self.value_type() {
            ValueType::String | ValueType::Data => self.get_data(),
            _ => &[],
        }
    }

    pub fn as_str(&self) -> &str {
        match self.value_type() {
            ValueType::String => std::str::from_utf8(self.get_data()).unwrap_or(""),
            _ => "",
        }
    }

    fn get_data(&self) -> &[u8] {
        if self.bytes.is_empty() {
            return &[];
        }
        let size = self.bytes[0] & 0x0F;
        if size == 0x0F {
            // varint
            let (bytes_read, size) = self.get_varint();
            if bytes_read == 0 {
                return &[];
            }
            let end = 1 + bytes_read + size as usize;
            &self.bytes[1 + bytes_read..end]
        } else {
            let end = 1 + size as usize;
            &self.bytes[1..end]
        }
    }

    // Return (bytes_read, size)
    pub fn get_varint(&self) -> (usize, u64) {
        if self.bytes.len() < 2 {
            return (0, 0);
        }

        if self.bytes.len() == 2 {
            return (1, self.bytes[1] as u64);
        }

        let mut shift = 0;
        let mut res = 0_u64;

        let end: usize = self.bytes.len().min(VARINT_MAX_LEN + 1);

        for (i, byte) in self.bytes[1..end].iter().enumerate() {
            if *byte >= 0x80 {
                res |= ((*byte & 0x7F) as u64) << shift;
                shift += 7;
            } else {
                res |= (*byte as u64) << shift;
                // Make sure the varint is below the max length
                if i == VARINT_MAX_LEN && *byte > 1 {
                    return (0, 0);
                }
                return (i + 1, res);
            }
        }

        (0, 0)
    }
}

// Arrays & Dicts
impl RawValue {
    pub fn arr_is_wide(&self) -> bool {
        self.bytes[0] & 0x08 != 0
    }

    pub fn arr_len(&self) -> usize {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(&self.bytes[0..2]);
        (u16::from_be_bytes(buf) & 0x07FF) as usize
    }
}

// Pointers
impl RawValue {
    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }

    // This should only be called when the data has already been validated
    unsafe fn ptr_deref<const WIDE: bool>(&self) -> &RawValue {
        let offset = self.ptr_get_offset::<WIDE>();
        debug_assert_ne!(offset, 0);

        let target_ptr = unsafe { self.ptr_offset(-(offset as isize)) };

        let target = self.from_raw_unchecked(target_ptr, offset);

        if target.value_type() == ValueType::Pointer {
            return target.ptr_deref::<true>();
        } else {
            target
        }
    }

    fn ptr_deref_untrusted(
        &self,
        wide: bool,
        data_start: *const u8,
        data_end: *const u8,
    ) -> Option<&RawValue> {
        if wide {
            if self.bytes.len() < 4 {
                return None;
            }
        } else if self.bytes.len() < 2 {
            return None;
        }

        let offset = unsafe {
            if wide {
                self.ptr_get_offset::<true>()
            } else {
                self.ptr_get_offset::<false>()
            }
        };
        if offset == 0 {
            return None;
        }

        // First get the pointer given by offset, so we can validate before dereferencing
        let target_ptr = unsafe { self.ptr_offset(-(offset as isize)) };

        // Is this pointer external to the source data?
        if self.bytes[0] & 0x40 != 0 {
            // return resolve_external_pointer(target_ptr, data_start, data_end);
            unimplemented!()
        // If the pointer isn't external, it should fit within the source data
        } else if target_ptr < data_start || target_ptr >= data_end {
            return None;
        }

        let target = unsafe { self.from_raw_unchecked(target_ptr, offset) };

        if target.value_type() == ValueType::Pointer {
            return target.ptr_deref_untrusted(true, data_start, self.as_ptr());
        } else {
            Some(target)
        }
    }

    pub unsafe fn offset_unchecked(&self, count: isize, width: usize) -> &RawValue {
        let target_ptr = unsafe { self.ptr_offset(count) };
        self.from_raw_unchecked(target_ptr, width)
    }

    pub unsafe fn deref_unchecked(&self, width: usize) -> &RawValue {
        if width == 4 {
            return self.ptr_deref::<true>();
        } else {
            return self.ptr_deref::<false>();
        }
    }

    unsafe fn ptr_offset(&self, offset: isize) -> *const u8 {
        self.bytes.as_ptr().offset(offset)
    }

    // Converts a pointer to a RawValue reference, and validates its size
    fn from_raw(&self, ptr: *const u8, available_size: usize) -> Option<&RawValue> {
        let target: &RawValue = unsafe {
            let slice = std::slice::from_raw_parts(ptr, available_size);
            std::mem::transmute(slice)
        };
        if target.len() < 2 {
            return None;
        }

        let required_size = target.required_size();
        if required_size > available_size {
            return None;
        }

        Some(target)
    }

    unsafe fn from_raw_unchecked(&self, ptr: *const u8, available_size: usize) -> &RawValue {
        let slice = std::slice::from_raw_parts(ptr, available_size);
        std::mem::transmute(slice)
    }

    unsafe fn ptr_get_offset<const WIDE: bool>(&self) -> usize {
        if WIDE {
            let mut buf = [0u8; 4];
            buf.copy_from_slice(&self.bytes[0..4]);
            ((u32::from_be_bytes(buf) & !0xC0000000) << 1) as usize
        } else {
            let mut buf = [0u8; 2];
            buf.copy_from_slice(&self.bytes[0..2]);
            ((u16::from_be_bytes(buf) & !0xC000) << 1) as usize
        }
    }
}

// Mutability
impl RawValue {
    pub fn is_mutable(&self) -> bool {
        self.bytes.as_ptr() as usize & 1 != 0
    }
}

impl Display for RawValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.bytes.is_empty() {
            return write!(f, "Empty");
        }
        match self.value_type() {
            ValueType::Null => write!(f, "Null"),
            ValueType::Undefined => write!(f, "Undefined"),
            ValueType::False => write!(f, "False"),
            ValueType::True => write!(f, "True"),
            ValueType::UnsignedShort | ValueType::UnsignedInt => self.as_unsigned_int().fmt(f),
            ValueType::Short | ValueType::Int => self.as_int().fmt(f),
            ValueType::Float | ValueType::Double => self.as_float().fmt(f),
            ValueType::String => self.as_str().fmt(f),
            ValueType::Data => write!(f, "Data"),
            ValueType::Array => write!(f, "Array"),
            ValueType::Dict => write!(f, "Dict"),
            ValueType::Pointer => write!(f, "Pointer"),
        }
    }
}

// Null, Undefined, Bool are special values. 4 bits tag + 4 bits special value.
// Short is 4 bits tag + 12 bits int. (range -2048, 2047 inclusive)
// Int is between 1 and 8 bytes, + 1 byte header (H, I, I, I, I, I, I, I, I) (2 - 9)
// Header is 4 bits tag + 1 bit signed / unsigned + 3 bits size (actually size - 1)
// Float is 4 bytes + 1 byte header + empty byte (H, 0, F, F, F, F) (6)
// Header is 4 bits tag + 4 bits 0.
// Double is 8 bytes + 1 byte header + empty byte (H, 0, F, F, F, F, F, F, F, F) (10)
// Header is 4 bits tag + 1000.
// Small strings (0 or 1 bytes) are 4 bits tag + 4 bits size + 1 byte string.
// Strings with 2 <= size <= 14 are 4 bits tag + 4 bits size + x bytes string.
// Strings with size >= 15 are 4 bits tag + 1111 (to mark varint) + x bytes varint + x bytes string.
// Binary data is written the same as long strings.
// Pointer is 2 or 4 bytes. 2 bits tag.
