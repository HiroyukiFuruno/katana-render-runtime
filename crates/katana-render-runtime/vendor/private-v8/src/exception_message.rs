use crate::private_v8::scope::PinScope;
use crate::private_v8::{
  Context, Local, Message, String, Value, support::int,
};

unsafe extern "C" {
  fn v8__Message__GetSourceLine(
    this: *const Message,
    context: *const Context,
  ) -> *const String;
  fn v8__Message__GetScriptResourceName(this: *const Message) -> *const Value;
  fn v8__Message__GetLineNumber(
    this: *const Message,
    context: *const Context,
  ) -> int;
  fn v8__Message__GetStartColumn(this: *const Message) -> int;
}

impl Message {
  #[inline(always)]
  pub fn get_source_line<'s>(
    &self,
    scope: &PinScope<'s, '_>,
  ) -> Option<Local<'s, String>> {
    unsafe {
      scope.cast_local(|sd| {
        v8__Message__GetSourceLine(self, sd.get_current_context())
      })
    }
  }

  /// Returns the resource name for the script from where the function causing
  /// the error originates.
  #[inline(always)]
  pub fn get_script_resource_name<'s>(
    &self,
    scope: &PinScope<'s, '_>,
  ) -> Option<Local<'s, Value>> {
    unsafe { scope.cast_local(|_| v8__Message__GetScriptResourceName(self)) }
  }

  /// Returns the number, 1-based, of the line where the error occurred.
  #[inline(always)]
  pub fn get_line_number(&self, scope: &PinScope<'_, '_>) -> Option<usize> {
    let i = unsafe {
      v8__Message__GetLineNumber(self, &*scope.get_current_context())
    };
    if i < 0 { None } else { Some(i as usize) }
  }





  /// Returns the index within the line of the first character where
  /// the error occurred.
  #[inline(always)]
  pub fn get_start_column(&self) -> usize {
    unsafe { v8__Message__GetStartColumn(self) as usize }
  }



}
