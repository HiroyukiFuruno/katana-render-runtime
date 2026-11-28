#![allow(non_snake_case)]

#[path = "exception_message.rs"]
mod message;

use crate::private_v8::scope::PinScope;
use crate::private_v8::{
  Local, String, Value,
};

unsafe extern "C" {
  fn v8__Exception__Error(message: *const String) -> *const Value;
}

/// Create new error objects by calling the corresponding error object
/// constructor with the message.
#[derive(Debug)]
pub struct Exception;

impl Exception {
  #[inline(always)]
  pub fn error<'s>(
    scope: &PinScope<'s, '_>,
    message: Local<String>,
  ) -> Local<'s, Value> {
    Self::new_error_with(scope, message, v8__Exception__Error)
  }





  /// Internal helper to make the above error constructors less repetitive.
  #[inline(always)]
  fn new_error_with<'s>(
    scope: &PinScope<'s, '_>,
    message: Local<String>,
    contructor: unsafe extern "C" fn(*const String) -> *const Value,
  ) -> Local<'s, Value> {
    unsafe {
      scope.enter();
      let error = scope.cast_local(|_| (contructor)(&*message)).unwrap();
      scope.exit();
      error
    }
  }



}
