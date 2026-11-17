use super::{Context, Local, MaybeBool, Name, Object, PinScope, Value};

unsafe extern "C" {
  fn v8__Object__Set(
    this: *const Object,
    context: *const Context,
    key: *const Value,
    value: *const Value,
  ) -> MaybeBool;
  fn v8__Object__SetIndex(
    this: *const Object,
    context: *const Context,
    index: u32,
    value: *const Value,
  ) -> MaybeBool;
  fn v8__Object__CreateDataProperty(
    this: *const Object,
    context: *const Context,
    key: *const Name,
    value: *const Value,
  ) -> MaybeBool;
}

impl Object {
  /// Set only return Just(true) or Empty(), so if it should never fail, use
  /// result.Check().
  #[inline(always)]
  pub fn set(
    &self,
    scope: &PinScope<'_, '_>,
    key: Local<Value>,
    value: Local<Value>,
  ) -> Option<bool> {
    unsafe {
      v8__Object__Set(self, &*scope.get_current_context(), &*key, &*value)
    }
    .into()
  }


  /// Set only return Just(true) or Empty(), so if it should never fail, use
  /// result.Check().
  #[inline(always)]
  pub fn set_index(
    &self,
    scope: &PinScope<'_, '_>,
    index: u32,
    value: Local<Value>,
  ) -> Option<bool> {
    unsafe {
      v8__Object__SetIndex(self, &*scope.get_current_context(), index, &*value)
    }
    .into()
  }


  /// Implements CreateDataProperty (ECMA-262, 7.3.4).
  ///
  /// Defines a configurable, writable, enumerable property with the given value
  /// on the object unless the property already exists and is not configurable
  /// or the object is not extensible.
  ///
  /// Returns true on success.
  #[inline(always)]
  pub fn create_data_property(
    &self,
    scope: &PinScope<'_, '_>,
    key: Local<Name>,
    value: Local<Value>,
  ) -> Option<bool> {
    unsafe {
      v8__Object__CreateDataProperty(
        self,
        &*scope.get_current_context(),
        &*key,
        &*value,
      )
    }
    .into()
  }
}
