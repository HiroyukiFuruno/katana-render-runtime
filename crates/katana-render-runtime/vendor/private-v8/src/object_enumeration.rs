#[cfg(test)]
use super::{
  Context, Local, MaybeBool, Object, PinScope,
  Value,
};

#[cfg(test)]
unsafe extern "C" {
  fn v8__Object__Has(
    this: *const Object,
    context: *const Context,
    key: *const Value,
  ) -> MaybeBool;
}

#[cfg(test)]
impl Object {


  /* Calls the abstract operation HasProperty(O, P) described in ECMA-262, */
  /* 7.3.10. Returns true, if the object has the property, either own or on the */
  /* prototype chain. Interceptors, i.e., PropertyQueryCallbacks, are called if */
  /* present. */
  /* */
  /* This function has the same side effects as JavaScript's variable in object. */
  /* For example, calling this on a revoked proxy will throw an exception. */
  /* */
  /* Note: This function converts the key to a name, which possibly calls back */
  /* into JavaScript. */
  #[inline(always)]
  pub fn has(
    &self,
    scope: &PinScope<'_, '_>,
    key: Local<Value>,
  ) -> Option<bool> {
    unsafe { v8__Object__Has(self, &*scope.get_current_context(), &*key) }
      .into()
  }

}
