#[cfg(test)]
use super::{
  Context, Local, Name, Object, PinScope, Value,
};

#[cfg(test)]
unsafe extern "C" {
  fn v8__Object__GetOwnPropertyDescriptor(
    this: *const Object,
    context: *const Context,
    key: *const Name,
  ) -> *const Value;
}

#[cfg(test)]
impl Object {

  /// Implements Object.getOwnPropertyDescriptor(O, P), see
  /// https://tc39.es/ecma262/#sec-object.getownpropertydescriptor.
  #[inline]
  pub fn get_own_property_descriptor<'s>(
    &self,
    scope: &PinScope<'s, '_>,
    key: Local<Name>,
  ) -> Option<Local<'s, Value>> {
    unsafe {
      scope.cast_local(|sd| {
        v8__Object__GetOwnPropertyDescriptor(
          self,
          sd.get_current_context(),
          &*key,
        )
      })
    }
  }




}
