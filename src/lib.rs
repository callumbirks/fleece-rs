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
