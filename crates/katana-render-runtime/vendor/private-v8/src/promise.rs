use std::marker::PhantomData;

use crate::private_v8::Local;
use crate::private_v8::Promise;
use crate::private_v8::Value;
use crate::private_v8::scope::PinScope;

unsafe extern "C" {
  fn v8__Promise__State(this: *const Promise) -> PromiseState;
  fn v8__Promise__Result(this: *const Promise) -> *const Value;

}

#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub enum PromiseState {
  Pending,
  Fulfilled,
  Rejected,
}

/* WHY: v8__Promise__StateはV8の列挙値をそのまま返す。 */
const _: () = {
  assert!(size_of::<PromiseState>() == size_of::<std::ffi::c_int>());
  assert!(PromiseState::Pending as std::ffi::c_int == 0);
  assert!(PromiseState::Fulfilled as std::ffi::c_int == 1);
  assert!(PromiseState::Rejected as std::ffi::c_int == 2);
};

impl Promise {
  /// Returns the value of the [[PromiseState]] field.
  #[inline(always)]
  pub fn state(&self) -> PromiseState {
    unsafe { v8__Promise__State(self) }
  }



  /// Returns the content of the [[PromiseResult]] field. The Promise must not
  /// be pending.
  #[inline(always)]
  pub fn result<'s>(&self, scope: &PinScope<'s, '_>) -> Local<'s, Value> {
    unsafe { scope.cast_local(|_| v8__Promise__Result(self)) }.unwrap()
  }




}

const PROMISE_REJECT_MESSAGE_ABI_WORDS: usize = 3;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PromiseRejectMessage<'msg>(
  [usize; PROMISE_REJECT_MESSAGE_ABI_WORDS],
  PhantomData<&'msg ()>,
);

impl<'msg> PromiseRejectMessage<'msg> {


}
