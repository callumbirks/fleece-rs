#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod encoder;
mod raw;
mod value;

pub use value::Value;

#[cfg(test)]
mod tests;
