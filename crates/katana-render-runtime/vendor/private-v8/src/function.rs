use std::marker::PhantomData;
use std::ptr::NonNull;
use std::ptr::null;

use crate::private_v8::CallbackScope;
use crate::private_v8::Context;
use crate::private_v8::Function;
use crate::private_v8::Local;
use crate::private_v8::Value;
use crate::private_v8::isolate::RealIsolate;
use crate::private_v8::scope::PinScope;
use crate::private_v8::support::MapFnFrom;
use crate::private_v8::support::MapFnTo;
use crate::private_v8::support::ToCFn;
use crate::private_v8::support::UnitType;
use crate::private_v8::support::{Opaque, int};

unsafe extern "C" {
  fn v8__Function__New(
    context: *const Context,
    callback: FunctionCallback,
    data_or_null: *const Value,
    length: i32,
    constructor_behavior: ConstructorBehavior,
    side_effect_type: SideEffectType,
  ) -> *const Function;
  fn v8__Function__Call(
    this: *const Function,
    context: *const Context,
    recv: *const Value,
    argc: int,
    argv: *const *const Value,
  ) -> *const Value;


  fn v8__FunctionCallbackInfo__GetIsolate(
    this: *const FunctionCallbackInfo,
  ) -> *mut RealIsolate;
  fn v8__FunctionCallbackInfo__Get(
    this: *const FunctionCallbackInfo,
    index: int,
  ) -> *const Value;
  fn v8__FunctionCallbackInfo__Length(this: *const FunctionCallbackInfo)
  -> int;
  fn v8__FunctionCallbackInfo__GetReturnValue(
    this: *const FunctionCallbackInfo,
  ) -> usize;

  fn v8__ReturnValue__Value__Set(
    this: *mut RawReturnValue,
    value: *const Value,
  );
}

#[path = "function_call_policy.rs"]
mod call_policy;
use call_policy::{ConstructorBehavior, SideEffectType};

#[path = "function_return_value.rs"]
mod return_value;
use return_value::RawReturnValue;
pub use return_value::ReturnValue;
#[path = "function_callback_info.rs"]
mod callback_info;
pub use callback_info::{
  FunctionCallbackArguments, FunctionCallbackInfo,
};
#[path = "function_callback_mapping.rs"]
mod function_callback;
pub use function_callback::FunctionCallback;
#[path = "function_api.rs"]
mod api;
