use super::{ConstructorBehavior, Function, FunctionCallback, Local, MapFnTo, PinScope, SideEffectType, Value, int, null, v8__Function__Call, v8__Function__New};

impl Function {
  /// Create a function bound to the current context.
  #[inline(always)]
  pub fn new<'s>(
    scope: &mut PinScope<'s, '_>,
    callback: impl MapFnTo<FunctionCallback>,
  ) -> Option<Local<'s, Function>> {
    unsafe {
      scope.cast_local(|sd| {
        v8__Function__New(
          sd.get_current_context(),
          callback.map_fn_to(),
          null(),
          0,
          ConstructorBehavior::Allow,
          SideEffectType::HasSideEffect,
        )
      })
    }
  }

  /// Call a function in a context scope.
  #[inline]
  pub fn call<'s>(
    &self,
    scope: &PinScope<'s, '_>,
    recv: Local<Value>,
    args: &[Local<Value>],
  ) -> Option<Local<'s, Value>> {
    let args = Local::slice_into_raw(args);
    let argc = int::try_from(args.len()).unwrap();
    let argv = args.as_ptr();
    unsafe {
      scope.cast_local(|sd| {
        v8__Function__Call(self, sd.get_current_context(), &*recv, argc, argv)
      })
    }
  }
}
