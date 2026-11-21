use super::{Context, Local, MaybeBool, Object, PinScope, Private, Value};

unsafe extern "C" {
  fn v8__Object__GetPrivate(
    this: *const Object,
    context: *const Context,
    key: *const Private,
  ) -> *const Value;
  fn v8__Object__SetPrivate(
    this: *const Object,
    context: *const Context,
    key: *const Private,
    value: *const Value,
  ) -> MaybeBool;
}

impl Object {
  /// Functionality for private properties.
  /// This is an experimental feature, use at your own risk.
  /// Note: Private properties are not inherited. Do not rely on this, since it
  /// may change.
  #[inline(always)]
  pub fn get_private<'s>(
    &self,
    scope: &PinScope<'s, '_>,
    key: Local<Private>,
  ) -> Option<Local<'s, Value>> {
    unsafe {
      scope.cast_local(|sd| {
        v8__Object__GetPrivate(self, sd.get_current_context(), &*key)
      })
    }
  }

  /// Functionality for private properties.
  /// This is an experimental feature, use at your own risk.
  /// Note: Private properties are not inherited. Do not rely on this, since it
  /// may change.
  #[inline(always)]
  pub fn set_private(
    &self,
    scope: &PinScope<'_, '_>,
    key: Local<Private>,
    value: Local<Value>,
  ) -> Option<bool> {
    unsafe {
      v8__Object__SetPrivate(
        self,
        &*scope.get_current_context(),
        &*key,
        &*value,
      )
    }
    .into()
  }


}
