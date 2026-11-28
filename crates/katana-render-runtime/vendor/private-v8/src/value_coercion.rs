use crate::private_v8::Context;
use crate::private_v8::Local;
#[cfg(test)]
use crate::private_v8::Object;
use crate::private_v8::PinScope;
use crate::private_v8::String;
use crate::private_v8::Value;

unsafe extern "C" {
  fn v8__Value__StrictEquals(this: *const Value, that: *const Value) -> bool;
  fn v8__Value__ToString(
    this: *const Value,
    context: *const Context,
  ) -> *const String;
  #[cfg(test)]
  fn v8__Value__ToObject(
    this: *const Value,
    context: *const Context,
  ) -> *const Object;
}

impl Value {
  #[inline(always)]
  pub fn strict_equals(&self, that: Local<Value>) -> bool {
    unsafe { v8__Value__StrictEquals(self, &*that) }
  }

  #[inline(always)]
  pub fn to_string<'s>(
    &self,
    scope: &PinScope<'s, '_>,
  ) -> Option<Local<'s, String>> {
    unsafe {
      scope
        .cast_local(|sd| v8__Value__ToString(self, &*sd.get_current_context()))
    }
  }

  /// Convenience function not present in the original V8 API.
  #[inline(always)]
  pub fn to_rust_string_lossy(
    &self,
    scope: &PinScope<'_, '_>,
  ) -> std::string::String {
    self
      .to_string(scope)
      .map_or_else(std::string::String::new, |s| s.to_rust_string_lossy(scope))
  }


  #[cfg(test)]
  #[inline(always)]
  pub fn to_object<'s>(
    &self,
    scope: &PinScope<'s, '_>,
  ) -> Option<Local<'s, Object>> {
    unsafe {
      scope
        .cast_local(|sd| v8__Value__ToObject(self, &*sd.get_current_context()))
    }
  }




}
