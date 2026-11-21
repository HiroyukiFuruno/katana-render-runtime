use super::{
  FunctionCallbackInfo, Local, PhantomData,
  Value, v8__FunctionCallbackInfo__GetReturnValue,
  v8__ReturnValue__Value__Set,
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(super) struct RawReturnValue(pub(super) usize);

/* Note: the 'cb lifetime is required because the ReturnValue object must not */
/* outlive the FunctionCallbackInfo object from which it */
/* is derived. */
#[derive(Debug)]
pub struct ReturnValue<'cb, T = Value>(RawReturnValue, PhantomData<&'cb T>);

impl<T> Clone for ReturnValue<'_, T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for ReturnValue<'_, T> {}

impl<'cb> ReturnValue<'cb, Value> {
  #[inline(always)]
  pub fn from_function_callback_info(info: &'cb FunctionCallbackInfo) -> Self {
    Self(
      unsafe { RawReturnValue(v8__FunctionCallbackInfo__GetReturnValue(info)) },
      PhantomData,
    )
  }
}

impl<T> ReturnValue<'_, T>
where
  for<'s> Local<'s, T>: Into<Local<'s, Value>>,
{
  #[inline(always)]
  pub fn set(&mut self, value: Local<T>) {
    unsafe { v8__ReturnValue__Value__Set(&mut self.0, &*value.into()) }
  }
}
