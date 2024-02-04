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
