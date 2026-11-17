use crate::private_v8::Context;
use crate::private_v8::PinScope;
use crate::private_v8::Value;
use crate::private_v8::isolate::RealIsolate;
use crate::private_v8::scope::GetIsolate;
use crate::private_v8::support::Maybe;

unsafe extern "C" {
  fn v8__Value__NumberValue(
    this: *const Value,
    context: *const Context,
    out: *mut Maybe<f64>,
  );
  fn v8__Value__BooleanValue(
    this: *const Value,
    isolate: *mut RealIsolate,
  ) -> bool;
  fn v8__Value__GetHash(this: *const Value) -> u32;
}

impl Value {

  #[inline(always)]
  pub fn number_value(&self, scope: &PinScope<'_, '_>) -> Option<f64> {
    let mut out = Maybe::<f64>::default();
    unsafe {
      v8__Value__NumberValue(self, &*scope.get_current_context(), &mut out);
    };
    out.into()
  }




  #[inline(always)]
  pub fn boolean_value(&self, scope: &PinScope<'_, '_>) -> bool {
    unsafe { v8__Value__BooleanValue(self, scope.get_isolate_ptr()) }
  }

  /// Get the hash of this value. The hash is not guaranteed to be
  /// unique. For |Object| and |Name| instances the result is equal to
  /// |GetIdentityHash|. Hashes are not guaranteed to be stable across
  /// different isolates or processes.
  #[inline(always)]
  pub fn get_hash(&self) -> u32 {
    unsafe { v8__Value__GetHash(self) }
  }

}
