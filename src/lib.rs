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
    } => {unsafe {
        let mut encoder = Encoder::new();
        encoder.begin_dict().unwrap_unchecked();
        $(fleece!(insert encoder => $key: $val);)*
        encoder.end_dict().unwrap_unchecked();
        encoder.finish_value().to_dict().unwrap()
    }};

    [
        $($val:tt),* $(,)?
    ] => {unsafe {
        let mut encoder = Encoder::new();
        encoder.begin_array(10).unwrap_unchecked();
        $(fleece!(push encoder => $val);)*
        encoder.end_array().unwrap_unchecked();
        encoder.finish_value().to_array().unwrap_unchecked()
    }};

    (insert $encoder:expr => $key:literal: { $($inner_key:literal: $inner_val:tt),*$(,)? }) => {
        $encoder.write_key($key).unwrap_unchecked();
        $encoder.begin_dict().unwrap_unchecked();
        $(fleece!(insert $encoder => $inner_key: $inner_val);)*
        $encoder.end_dict().unwrap_unchecked();
    };

    (insert $encoder:expr => $key:literal: [ $($inner_val:tt),* $(,)? ]) => {
        $encoder.write_key($key).unwrap_unchecked();
        $encoder.begin_array(10).unwrap_unchecked();
        $(fleece!(push $encoder => $inner_val);)*
        $encoder.end_array().unwrap_unchecked();
    };

    (insert $encoder:expr => $key:literal: $val:expr) => {
        $encoder.write_key($key).unwrap_unchecked();
        $encoder.write_value($val).unwrap_unchecked();
    };

    (push $encoder:expr => { $($inner_key:literal: $inner_val:tt),* $(,)? }) => {
        $encoder.begin_dict().unwrap_unchecked();
        $(fleece!(insert $encoder => $inner_key: $inner_val);)*
        $encoder.end_dict().unwrap_unchecked();
    };

    (push $encoder:expr => [ $($inner_val:tt),* $(,)? ]) => {
        $encoder.begin_array().unwrap_unchecked();
        $(fleece!(push $encoder => $inner_val);)*
        $encoder.end_array().unwrap_unchecked();
    };

    (push $encoder:expr => $val:expr) => {
        $encoder.write_value($val).unwrap_unchecked();
    };
}
