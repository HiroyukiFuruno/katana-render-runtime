use crate::private_v8::Boolean;
use crate::private_v8::Local;
use crate::private_v8::Primitive;
use crate::private_v8::isolate::Isolate;
use crate::private_v8::isolate::RealIsolate;

unsafe extern "C" {
  fn v8__Null(isolate: *mut RealIsolate) -> *const Primitive;
  fn v8__Undefined(isolate: *mut RealIsolate) -> *const Primitive;

  fn v8__Boolean__New(isolate: *mut RealIsolate, value: bool)
  -> *const Boolean;
}

impl Primitive {
  #[inline(always)]
  pub fn null<'s, R>(scope: &R) -> Local<'s, Primitive>
  where
    R: AsRef<Isolate>,
  {
    unsafe { Local::from_raw_unchecked(v8__Null(scope.as_ref().as_real_ptr())) }
  }

  #[inline(always)]
  pub fn undefined<'s, R>(scope: &R) -> Local<'s, Primitive>
  where
    R: AsRef<Isolate>,
  {
    unsafe {
      Local::from_raw_unchecked(v8__Undefined(scope.as_ref().as_real_ptr()))
    }
  }
}

impl Boolean {
  #[inline(always)]
  pub fn new<'s, R>(scope: &R, value: bool) -> Local<'s, Boolean>
  where
    R: AsRef<Isolate>,
  {
    unsafe {
      Local::from_raw_unchecked(v8__Boolean__New(
        scope.as_ref().as_real_ptr(),
        value,
      ))
    }
  }
}
