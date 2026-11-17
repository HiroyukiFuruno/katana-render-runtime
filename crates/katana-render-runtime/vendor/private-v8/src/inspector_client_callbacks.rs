use super::*;
use super::inspector_client::V8InspectorClientHeap;
use crate::private_v8::CallbackScope;

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorClient__BASE__generateUniqueId(
  this: *mut RawV8InspectorClient,
) -> i64 {
  unsafe {
    V8InspectorClientHeap::from_raw(this)
      .imp
      .generate_unique_id()
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorClient__BASE__runMessageLoopOnPause(
  this: *mut RawV8InspectorClient,
  context_group_id: int,
) {
  unsafe {
    V8InspectorClientHeap::from_raw(this)
      .imp
      .run_message_loop_on_pause(context_group_id);
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorClient__BASE__quitMessageLoopOnPause(
  this: *mut RawV8InspectorClient,
) {
  unsafe {
    V8InspectorClientHeap::from_raw(this)
      .imp
      .quit_message_loop_on_pause();
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorClient__BASE__runIfWaitingForDebugger(
  this: *mut RawV8InspectorClient,
  context_group_id: int,
) {
  unsafe {
    V8InspectorClientHeap::from_raw(this)
      .imp
      .run_if_waiting_for_debugger(context_group_id);
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorClient__BASE__consoleAPIMessage(
  this: *mut RawV8InspectorClient,
  context_group_id: int,
  level: int,
  message: &StringView,
  url: &StringView,
  line_number: u32,
  column_number: u32,
  stack_trace: &mut V8StackTrace,
) {
  unsafe {
    V8InspectorClientHeap::from_raw(this)
      .imp
      .console_api_message(
        context_group_id,
        level,
        message,
        url,
        line_number,
        column_number,
        stack_trace,
      );
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorClient__BASE__valueSubtype(
  this: *mut RawV8InspectorClient,
  context: Local<Context>,
  value: Local<Value>,
) -> *mut StringBuffer {
  let scope = pin!(unsafe { CallbackScope::new(context) });
  let mut scope = scope.init();
  unsafe {
    V8InspectorClientHeap::from_raw(this)
      .imp
      .value_subtype(&mut scope, value)
      .and_then(|mut v| v.take())
      .map(|r| r.into_raw())
      .unwrap_or(std::ptr::null_mut())
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorClient__BASE__descriptionForValueSubtype(
  this: *mut RawV8InspectorClient,
  context: Local<Context>,
  value: Local<Value>,
) -> *mut StringBuffer {
  let scope = pin!(unsafe { CallbackScope::new(context) });
  let mut scope = scope.init();
  unsafe {
    V8InspectorClientHeap::from_raw(this)
      .imp
      .description_for_value_subtype(&mut scope, value)
      .and_then(|mut v| v.take())
      .map(|r| r.into_raw())
      .unwrap_or(std::ptr::null_mut())
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorClient__BASE__ensureDefaultContextInGroup(
  this: *mut RawV8InspectorClient,
  context_group_id: int,
) -> *const Context {
  unsafe {
    match V8InspectorClientHeap::from_raw(this)
      .imp
      .ensure_default_context_in_group(context_group_id)
    {
      Some(h) => &*h,
      None => std::ptr::null_mut(),
    }
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorClient__BASE__resourceNameToUrl(
  this: *mut RawV8InspectorClient,
  resource_name: &StringView,
) -> *mut StringBuffer {
  unsafe {
    V8InspectorClientHeap::from_raw(this)
      .imp
      .resource_name_to_url(resource_name)
      .and_then(|mut v| v.take())
      .map(|r| r.into_raw())
      .unwrap_or(std::ptr::null_mut())
  }
}
