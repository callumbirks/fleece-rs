mod encoder;
mod scope;
mod sharedkeys;
mod value;

// Example of modules
//#[cfg(feature = "datetime")]
//mod datetime;
//#[cfg(feature = "serde")]

pub use encoder::Encoder;
pub use value::Value;

#[inline]
#[cold]
pub(crate) fn cold() {}

#[inline]
pub(crate) fn likely(b: bool) -> bool {
    if !b {
        cold();
    }
    b
}

#[inline]
pub(crate) fn unlikely(b: bool) -> bool {
    if b {
        cold();
    }
    b
}

#[cfg(test)]
mod tests;
