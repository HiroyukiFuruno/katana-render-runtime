use std::alloc::Layout;
use std::ptr::NonNull;

use crate::private_v8::Integer;
use crate::private_v8::Local;
use crate::private_v8::Number;
use crate::private_v8::isolate::RealIsolate;
use crate::private_v8::scope::PinScope;

unsafe extern "C" {
  fn v8__Number__New(isolate: *mut RealIsolate, value: f64) -> *const Number;
  fn v8__Integer__Value(this: *const Integer) -> i64;
}

impl Number {
  #[inline(always)]
  pub fn new<'s>(
    scope: &PinScope<'s, '_, ()>,
    value: f64,
  ) -> Local<'s, Number> {
    unsafe {
      scope.cast_local(|sd| v8__Number__New(sd.get_isolate_ptr(), value))
    }
    .unwrap()
  }

}

impl Integer {
  #[inline(always)]
  pub fn value(&self) -> i64 {
    unsafe { v8__Integer__Value(self) }
  }

  /// Internal helper function to produce a handle containing a SMI zero value,
  /// without the need for the caller to provide (or have entered) a
  /// `HandleScope`.
  #[inline(always)]
  pub(crate) fn zero<'s>() -> Local<'s, Integer> {
    /* The SMI representation of zero is also zero. In debug builds, double */
    /* check this, so in the unlikely event that V8 changes its internal */
    /* representation of SMIs such that this invariant no longer holds, we'd */
    /* catch it. */
    static ZERO_SMI: usize = 0;
    let zero_raw = &ZERO_SMI as *const _ as *mut Self;
    let zero_nn = unsafe { NonNull::new_unchecked(zero_raw) };
    let zero_local = unsafe { Local::from_non_null(zero_nn) };
    debug_assert_eq!(Layout::new::<usize>(), Layout::new::<Local<Self>>());
    debug_assert_eq!(zero_local.value(), 0);
    zero_local
  }
}
