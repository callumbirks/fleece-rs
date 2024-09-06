use lazy_static::lazy_static;

use crate::{value, Array, Dict, Value, ValueType};
use std::borrow::Borrow;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Alloced<T>
where
    T: ?Sized,
{
    pub(crate) buf: Pin<Arc<[u8]>>,
    pub(crate) value: *const T,
}

impl<T: ?Sized> Alloced<T> {
    #[must_use]
    pub fn value(&self) -> &T {
        unsafe { &*self.value }
    }
}

pub type AllocedValue = Alloced<Value>;
pub type AllocedDict = Alloced<Dict>;
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
            buf: Pin::new(Arc::from(data.to_vec().into_boxed_slice())),
            value: std::ptr::slice_from_raw_parts(NonNull::<u8>::dangling().as_ptr(), 0)
                as *const Value,
        }
    }
}

lazy_static! {
    static ref EMPTY_ARRAY: Pin<Arc<[u8]>> = Arc::pin([value::tag::ARRAY, 0]);
    static ref EMPTY_DICT: Pin<Arc<[u8]>> = Arc::pin([value::tag::DICT, 0]);
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
