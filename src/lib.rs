#![cfg_attr(not(test), no_std)]

#[macro_use]
extern crate alloc;

pub mod alloced;
#[cfg(feature = "serde")]
mod de;
pub mod encoder;
pub mod error;
pub mod mutable;
mod scope;
#[cfg(feature = "serde")]
mod ser;
pub mod shared_keys;
#[cfg(test)]
mod tests;
pub mod value;

#[cfg(feature = "serde")]
pub use de::from_bytes;
#[cfg(feature = "serde")]
pub use de::Deserializer;
pub use encoder::Encoder;
pub use error::Error;
pub use error::Result;
pub use mutable::MutableArray;
pub use mutable::MutableDict;
pub use scope::Scope;
#[cfg(feature = "serde")]
pub use ser::to_bytes;
#[cfg(feature = "serde")]
pub use ser::to_bytes_with_shared_keys;
#[cfg(feature = "serde")]
pub use ser::Serializer;
pub use shared_keys::SharedKeys;
pub use value::array::Array;
pub use value::dict::Dict;
pub use value::Value;
pub use value::ValueType;

#[macro_export]
macro_rules! fleece {
    {
        $($key:literal: $val:tt),* $(,)?
    } => {{
        let mut encoder = Encoder::new();
        unsafe { encoder.begin_dict().unwrap_unchecked() };
        $(fleece!(insert encoder => $key: $val);)*
        unsafe { encoder.end_dict().unwrap_unchecked() };
        unsafe { encoder.finish_value().to_dict().unwrap_unchecked() }
    }};

    [
        $($val:tt),* $(,)?
    ] => {{
        let mut encoder = Encoder::new();
        unsafe { encoder.begin_array(10).unwrap_unchecked() };
        $(fleece!(push encoder => $val);)*
        unsafe { encoder.end_array().unwrap_unchecked() };
        unsafe { encoder.finish_value().to_array().unwrap_unchecked() }
    }};

    (insert $encoder:expr => $key:literal: { $($inner_key:literal: $inner_val:tt),*$(,)? }) => {
        let Ok(_) = $encoder.write_key($key) else {
            unsafe { core::hint::unreachable_unchecked() }
        };
        unsafe { $encoder.begin_dict().unwrap_unchecked() };
        $(fleece!(insert $encoder => $inner_key: $inner_val);)*
        unsafe { $encoder.end_dict().unwrap_unchecked() };
    };

    (insert $encoder:expr => $key:literal: [ $($inner_val:tt),* $(,)? ]) => {
        let Ok(_) = $encoder.write_key($key) else {
            unsafe { core::hint::unreachable_unchecked() }
        };
        unsafe { $encoder.begin_array(10).unwrap_unchecked() };
        $(fleece!(push $encoder => $inner_val);)*
        unsafe { $encoder.end_array().unwrap_unchecked() };
    };

    (insert $encoder:expr => $key:literal: $val:expr) => {
        let Ok(_) = $encoder.write_key($key) else {
            unsafe { core::hint::unreachable_unchecked() }
        };
        let Ok(_) = $encoder.write_value($val) else {
            unsafe { core::hint::unreachable_unchecked() }
        };
    };

    (push $encoder:expr => { $($inner_key:literal: $inner_val:tt),* $(,)? }) => {
        unsafe { $encoder.begin_dict().unwrap_unchecked() };
        $(fleece!(insert $encoder => $inner_key: $inner_val);)*
        unsafe { $encoder.end_dict().unwrap_unchecked() };
    };

    (push $encoder:expr => [ $($inner_val:tt),* $(,)? ]) => {
        unsafe { $encoder.begin_array().unwrap_unchecked() };
        $(fleece!(push $encoder => $inner_val);)*
        unsafe { $encoder.end_array().unwrap_unchecked() };
    };

    (push $encoder:expr => $val:expr) => {
        let Ok(_) = $encoder.write_value($val) else {
            unsafe { core::hint::unreachable_unchecked() }
        };
    };
}
