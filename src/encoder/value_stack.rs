use crate::raw::sized::SizedValue;

pub struct ValueArray {
    keys: Vec<Box<str>>,
    values: Vec<SizedValue>
}