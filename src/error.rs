use crate::encoder::EncodeError;
use crate::value::DecodeError;
use alloc::string::String;
use core::fmt;

#[cfg(feature = "serde")]
pub use crate::de::DeserializeError;
#[cfg(feature = "serde")]
pub use crate::ser::SerializeError;

#[derive(Debug)]
pub enum Error {
    Encode(EncodeError),
    Decode(DecodeError),
    Message(String),
    #[cfg(feature = "serde")]
    Serialize(SerializeError),
    #[cfg(feature = "serde")]
    Deserialize(DeserializeError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Encode(e) => write!(f, "Encode {e}"),
            Error::Decode(e) => write!(f, "Decode {e}"),
            Error::Message(m) => write!(f, "{m}"),
            #[cfg(feature = "serde")]
            Error::Serialize(e) => write!(f, "Serialize {e}"),
            #[cfg(feature = "serde")]
            Error::Deserialize(e) => write!(f, "Deserialize {e}"),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::ser::StdError for Error {}

impl From<EncodeError> for Error {
    fn from(value: EncodeError) -> Self {
        Error::Encode(value)
    }
}

impl From<DecodeError> for Error {
    fn from(value: DecodeError) -> Self {
        Error::Decode(value)
    }
}

#[cfg(feature = "serde")]
impl From<SerializeError> for Error {
    fn from(value: SerializeError) -> Self {
        Error::Serialize(value)
    }
}

#[cfg(feature = "serde")]
impl From<DeserializeError> for Error {
    fn from(value: DeserializeError) -> Self {
        Error::Deserialize(value)
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[cfg(feature = "serde")]
impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        use alloc::string::ToString;
        Error::Message(msg.to_string())
    }
}

#[cfg(feature = "serde")]
impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        use alloc::string::ToString;
        Error::Message(msg.to_string())
    }
}
