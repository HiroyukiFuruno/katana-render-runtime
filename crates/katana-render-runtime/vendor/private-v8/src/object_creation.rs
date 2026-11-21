use super::{
  Local, NonZeroI32, Object, PinScope, RealIsolate, int,
};

unsafe extern "C" {
  fn v8__Object__New(isolate: *mut RealIsolate) -> *const Object;
  fn v8__Object__GetIdentityHash(this: *const Object) -> int;
}

impl Object {
  /// Creates an empty object.
  #[inline(always)]
  pub fn new<'s>(scope: &PinScope<'s, '_>) -> Local<'s, Object> {
    unsafe { scope.cast_local(|sd| v8__Object__New(sd.get_isolate_ptr())) }
      .unwrap()
  }



  /// Returns the V8 hash value for this value. The current implementation
  /// uses a hidden property to store the identity hash.
  ///
  /// The return value will never be 0. Also, it is not guaranteed to be
  /// unique.
  #[inline(always)]
  pub fn get_identity_hash(&self) -> NonZeroI32 {
    unsafe { NonZeroI32::new_unchecked(v8__Object__GetIdentityHash(self)) }
  }

}
