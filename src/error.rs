use crate::encoder::EncodeError;
use crate::value::DecodeError;
use std::fmt::Debug;
use thiserror::Error;

#[cfg(feature = "serde")]
pub use crate::de::DeserializeError;
#[cfg(feature = "serde")]
pub use crate::ser::SerializeError;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Encode {0}")]
    Encode(#[from] EncodeError),
    #[error("Decode {0}")]
    Decode(#[from] DecodeError),
    #[error("{0}")]
    Message(String),
    #[cfg(feature = "serde")]
    #[error("Serialize {0}")]
    Serialize(#[from] SerializeError),
    #[cfg(feature = "serde")]
    #[error("Deserialize")]
    Deserialize(#[from] DeserializeError),
}

pub type Result<T> = core::result::Result<T, Error>;

#[cfg(feature = "serde")]
impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error::Message(msg.to_string())
    }
}

#[cfg(feature = "serde")]
impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error::Message(msg.to_string())
    }
}
