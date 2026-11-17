use super::{
  CallbackScope, FunctionCallbackArguments, FunctionCallbackInfo, MapFnFrom,
  PinScope, ReturnValue, ToCFn, UnitType, Value,
};

pub type FunctionCallback = unsafe extern "C" fn(*const FunctionCallbackInfo);

impl<F> MapFnFrom<F> for FunctionCallback
where
  F: UnitType
    + for<'s, 'i> Fn(
      &mut PinScope<'s, 'i>,
      FunctionCallbackArguments<'s>,
      ReturnValue<'s, Value>,
    ),
{
  fn mapping() -> Self {
    let f = |info: *const FunctionCallbackInfo| {
      let info = unsafe { &*info };
      let scope = std::pin::pin!(unsafe { CallbackScope::new(info) });
      let mut scope = scope.init();
      let args = FunctionCallbackArguments::from_function_callback_info(info);
      let rv = ReturnValue::from_function_callback_info(info);
      (F::get())(&mut scope, args, rv);
    };
    f.to_c_fn()
  }
}
