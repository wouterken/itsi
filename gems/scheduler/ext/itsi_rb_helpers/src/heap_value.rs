use magnus::IntoValue;
use magnus::rb_sys::AsRawValue;
use magnus::value::BoxValue;
use magnus::{Ruby, Value, value::ReprValue};
use std::fmt::{self, Debug, Formatter};
use std::ops::Deref;

/// HeapVal is a wrapper for heap-allocated magnus ReprVa;ies
/// that is marked as thread-safe(Send and Sync)
/// It's up to the user to actually ensure this though,
/// typically by only interacting with the value from a thread which
/// holds the GVL.
pub struct HeapValue<T>(pub BoxValue<T>)
where
    T: ReprValue;

impl<T> PartialEq for HeapValue<T>
where
    T: ReprValue,
{
    fn eq(&self, other: &Self) -> bool {
        self.0.as_raw() == other.0.as_raw()
    }
}

impl<T> Deref for HeapValue<T>
where
    T: ReprValue,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> HeapValue<T>
where
    T: ReprValue,
{
    pub fn inner(self) -> T {
        *self.0
    }
}

impl<T> IntoValue for HeapValue<T>
where
    T: ReprValue,
{
    fn into_value_with(self, _: &Ruby) -> Value {
        self.0.into_value()
    }
}

impl<T> From<T> for HeapValue<T>
where
    T: ReprValue,
{
    fn from(value: T) -> Self {
        HeapValue(BoxValue::new(value))
    }
}

impl<T> Clone for HeapValue<T>
where
    T: ReprValue + Clone,
{
    fn clone(&self) -> Self {
        HeapValue(BoxValue::new(*self.0.deref()))
    }
}

impl<T> Debug for HeapValue<T>
where
    T: ReprValue + Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

unsafe impl<T> Send for HeapValue<T> where T: ReprValue {}
unsafe impl<T> Sync for HeapValue<T> where T: ReprValue {}

/// HeapVal is a wrapper for heap-allocated magnus Values
/// that is marked as thread-safe(Send and Sync)
/// It's up to the user to actually ensure this though,
/// typically by only interacting with the value from a thread which
/// holds the GVL.
pub struct HeapVal(HeapValue<Value>);
impl Deref for HeapVal {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoValue for HeapVal {
    fn into_value_with(self, _: &Ruby) -> Value {
        self.0.into_value()
    }
}

impl From<Value> for HeapVal {
    fn from(value: Value) -> Self {
        HeapVal(HeapValue(BoxValue::new(value)))
    }
}

impl Clone for HeapVal {
    fn clone(&self) -> Self {
        HeapVal(HeapValue(BoxValue::new(*self.0.deref())))
    }
}

impl Debug for HeapVal {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
