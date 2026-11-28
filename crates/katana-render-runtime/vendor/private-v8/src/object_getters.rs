use super::{Context, Local, Object, PinScope, Value};

unsafe extern "C" {
  fn v8__Object__Get(
    this: *const Object,
    context: *const Context,
    key: *const Value,
  ) -> *const Value;
  fn v8__Object__GetIndex(
    this: *const Object,
    context: *const Context,
    index: u32,
  ) -> *const Value;
}

impl Object {
  #[inline(always)]
  pub fn get<'s>(
    &self,
    scope: &PinScope<'s, '_>,
    key: Local<Value>,
  ) -> Option<Local<'s, Value>> {
    unsafe {
      scope
        .cast_local(|sd| v8__Object__Get(self, sd.get_current_context(), &*key))
    }
  }


  #[inline(always)]
  pub fn get_index<'s>(
    &self,
    scope: &PinScope<'s, '_>,
    index: u32,
  ) -> Option<Local<'s, Value>> {
    unsafe {
      scope.cast_local(|sd| {
        v8__Object__GetIndex(self, sd.get_current_context(), index)
      })
    }
  }

}
