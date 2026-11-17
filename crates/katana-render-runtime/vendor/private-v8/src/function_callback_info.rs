use super::{
  Local, NonNull, Opaque, RealIsolate, Value, int, v8__FunctionCallbackInfo__Get,
  v8__FunctionCallbackInfo__GetIsolate, v8__FunctionCallbackInfo__Length,
};

/// The argument information given to function call callbacks.  This
/// class provides access to information about the context of the call,
/// including the number and values of arguments.
#[repr(C)]
#[derive(Debug)]
pub struct FunctionCallbackInfo(*mut Opaque);

impl FunctionCallbackInfo {
  #[inline(always)]
  pub(crate) fn get_isolate_ptr(&self) -> *mut RealIsolate {
    unsafe { v8__FunctionCallbackInfo__GetIsolate(self) }
  }



  #[inline(always)]
  pub(crate) fn length(&self) -> int {
    unsafe { v8__FunctionCallbackInfo__Length(self) }
  }

  #[inline(always)]
  pub(crate) fn get(&self, index: int) -> Local<'_, Value> {
    unsafe {
      let ptr = v8__FunctionCallbackInfo__Get(self, index);
      let nn = NonNull::new_unchecked(ptr as *mut Value);
      Local::from_non_null(nn)
    }
  }
}

#[derive(Debug)]
pub struct FunctionCallbackArguments<'s> {
  info: &'s FunctionCallbackInfo,
}

impl<'s> FunctionCallbackArguments<'s> {
  #[inline(always)]
  pub fn from_function_callback_info(info: &'s FunctionCallbackInfo) -> Self {
    Self { info }
  }


  /// The number of available arguments.
  #[inline(always)]
  pub fn length(&self) -> int {
    self.info.length()
  }

  /// Accessor for the available arguments. Returns `undefined` if the index is
  /// out of bounds.
  #[inline(always)]
  pub fn get(&self, i: int) -> Local<'s, Value> {
    self.info.get(i)
  }
}
