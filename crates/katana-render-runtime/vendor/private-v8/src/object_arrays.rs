use super::{Array, Local, PinScope, RealIsolate, int};

unsafe extern "C" {
  fn v8__Array__New(isolate: *mut RealIsolate, length: int) -> *const Array;
  fn v8__Array__Length(array: *const Array) -> u32;
}

impl Array {
  /// Creates a JavaScript array with the given length. If the length
  /// is negative the returned array will have length 0.
  #[inline(always)]
  pub fn new<'s>(scope: &PinScope<'s, '_>, length: i32) -> Local<'s, Array> {
    unsafe {
      scope.cast_local(|sd| v8__Array__New(sd.get_isolate_ptr(), length))
    }
    .unwrap()
  }


  #[inline(always)]
  pub fn length(&self) -> u32 {
    unsafe { v8__Array__Length(self) }
  }
}
