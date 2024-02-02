#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod raw;
mod value;
mod encoder;

// Example of modules
//#[cfg(feature = "datetime")]
//mod datetime;
//#[cfg(feature = "serde")]

pub use value::Value;

#[cfg(test)]
mod tests;
mod sharedkeys;