use lazy_static::lazy_static;

use crate::{value, Array, Dict, Value, ValueType};
use core::fmt;
use std::borrow::Borrow;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::Arc;

/// A [`Value`] which manages its own memory. This can be constructed with [`Value::from_bytes_alloced`].
/// If you have an [`AllocedValue`] and need an [`AllocedArray`] or [`AllocedDict`], you can use
/// [`AllocedValue::to_array`] or [`AllocedValue::to_dict`] respectively.
#[derive(Clone)]
pub struct Alloced<T>
where
    T: ?Sized,
{
    pub(crate) buf: Arc<[u8]>,
    pub(crate) value: *const T,
}

impl<T: ?Sized> Alloced<T> {
    #[must_use]
    pub fn value(&self) -> &T {
        unsafe { &*self.value }
    }
}

/// A [`Value`] which manages its own memory. This can be constructed with [`Value::clone_from_bytes`].
/// If you have an [`AllocedValue`] and need an [`AllocedArray`] or [`AllocedDict`], you can use
/// [`AllocedValue::to_array`] or [`AllocedValue::to_dict`] respectively.
pub type AllocedValue = Alloced<Value>;
/// A [`Dict`] which manages its own memory. This can be constructed with [`Dict::clone_from_bytes`].
pub type AllocedDict = Alloced<Dict>;
/// An [`Array`] which manages its own memory. This can be constructed with [`Array::clone_from_bytes`].
pub type AllocedArray = Alloced<Array>;

impl AllocedValue {
    /// Convert this to an [`AllocedArray`]. Returns `None` if the inner [`Value`] is not an
    /// [`Array`].
    #[must_use]
    pub fn to_array(self) -> Option<AllocedArray> {
        if self.value_type() == ValueType::Array {
            Some(AllocedArray {
                buf: self.buf,
                value: std::ptr::from_ref(Array::from_value(unsafe { &*self.value })),
            })
        } else {
            None
        }
    }

    /// Convert this to an [`AllocedDict`]. Returns `None` if the inner [`Value`] is not a [`Dict`].
    #[must_use]
    pub fn to_dict(self) -> Option<AllocedDict> {
        if self.value_type() == ValueType::Dict {
            Some(AllocedDict {
                buf: self.buf,
                value: std::ptr::from_ref(Dict::from_value(unsafe { &*self.value })),
            })
        } else {
            None
        }
    }

    pub(crate) unsafe fn new_dangling(data: &[u8]) -> Self {
        Self {
            buf: Arc::from(data.to_vec()),
            value: std::ptr::slice_from_raw_parts(NonNull::<u8>::dangling().as_ptr(), 0)
                as *const Value,
        }
    }
}

lazy_static! {
    static ref EMPTY_ARRAY: Arc<[u8]> = Arc::new([value::tag::ARRAY, 0]);
    static ref EMPTY_DICT: Arc<[u8]> = Arc::new([value::tag::DICT, 0]);
}

impl AllocedArray {
    /// An empty [`AllocedArray`]. Doesn't perform any allocation because it points to a constant.
    #[must_use]
    pub fn empty() -> Self {
        AllocedArray {
            buf: EMPTY_ARRAY.clone(),
            value: std::ptr::slice_from_raw_parts(EMPTY_ARRAY.as_ptr(), EMPTY_ARRAY.len())
                as *const Array,
        }
    }
}

impl AllocedDict {
    /// An empty [`AllocedDict`]. Doesn't perform any allocation because it points to a constant.
    #[must_use]
    pub fn empty() -> Self {
        AllocedDict {
            buf: EMPTY_DICT.clone(),
            value: std::ptr::slice_from_raw_parts(EMPTY_DICT.as_ptr(), EMPTY_DICT.len())
                as *const Dict,
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Alloced<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Alloced")
            .field("buf", &self.buf)
            .field("value_ptr", &self.value)
            .field("value", &self.value())
            .finish()
    }
}

impl<T> AsRef<T> for Alloced<T>
where
    T: ?Sized,
{
    fn as_ref(&self) -> &T {
        self.value()
    }
}

impl<T> Borrow<T> for Alloced<T>
where
    T: ?Sized,
{
    fn borrow(&self) -> &T {
        self.value()
    }
}

impl<T: ?Sized> Deref for Alloced<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value()
    }
}

unsafe impl<T: ?Sized> Send for Alloced<T> {}
unsafe impl<T: ?Sized> Sync for Alloced<T> {}
