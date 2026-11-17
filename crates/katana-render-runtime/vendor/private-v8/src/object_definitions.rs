use super::{
  Context, Local, MaybeBool, Name, Object, PinScope, PropertyAttribute, Value,
};
#[cfg(test)]
use super::IntegrityLevel;

unsafe extern "C" {
  fn v8__Object__DefineOwnProperty(
    this: *const Object,
    context: *const Context,
    key: *const Name,
    value: *const Value,
    attr: PropertyAttribute,
  ) -> MaybeBool;
  #[cfg(test)]
  fn v8__Object__SetIntegrityLevel(
    this: *const Object,
    context: *const Context,
    level: IntegrityLevel,
  ) -> MaybeBool;
}

impl Object {
  /// Implements DefineOwnProperty.
  ///
  /// In general, CreateDataProperty will be faster, however, does not allow
  /// for specifying attributes.
  ///
  /// Returns true on success.
  #[inline(always)]
  pub fn define_own_property(
    &self,
    scope: &PinScope<'_, '_>,
    key: Local<Name>,
    value: Local<Value>,
    attr: PropertyAttribute,
  ) -> Option<bool> {
    unsafe {
      v8__Object__DefineOwnProperty(
        self,
        &*scope.get_current_context(),
        &*key,
        &*value,
        attr,
      )
    }
    .into()
  }


  /// Sets the integrity level of the object.
  #[cfg(test)]
  #[inline(always)]
  pub fn set_integrity_level(
    &self,
    scope: &PinScope<'_, '_>,
    level: IntegrityLevel,
  ) -> Option<bool> {
    unsafe {
      v8__Object__SetIntegrityLevel(self, &*scope.get_current_context(), level)
    }
    .into()
  }
}
