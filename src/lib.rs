#[cfg(feature = "serde")]
mod de;
pub mod encoder;
pub mod error;
#[cfg(feature = "serde")]
mod ser;
#[cfg(test)]
mod tests;
pub mod value;

pub use encoder::Encoder;
pub use error::Error;
pub use error::Result;
pub use value::array::Array;
pub use value::dict::Dict;
pub use value::Value;
pub use value::ValueType;
#[cfg(feature = "serde")]
pub use de::Deserializer;
#[cfg(feature = "serde")]
pub use de::from_bytes;
#[cfg(feature = "serde")]
pub use ser::Serializer;
#[cfg(feature = "serde")]
pub use ser::to_bytes;
#[cfg(feature = "serde")]
pub use ser::to_writer;
