use crate::{Array, Dict, Value, ValueType};
use std::borrow::Borrow;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::NonNull;

#[derive(Debug, Clone)]
pub struct Alloced<T>
where
    T: ?Sized,
{
    pub(crate) buf: Pin<Box<[u8]>>,
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
            buf: Pin::from(data.to_vec().into_boxed_slice()),
            value: std::ptr::slice_from_raw_parts(NonNull::<u8>::dangling().as_ptr(), 0)
                as *const Value,
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
