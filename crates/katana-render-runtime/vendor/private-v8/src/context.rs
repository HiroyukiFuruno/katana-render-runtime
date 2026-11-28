
use crate::private_v8::isolate::RealIsolate;
use crate::private_v8::scope::PinScope;
/* Copyright 2019-2021 the Deno authors. All rights reserved. MIT license. */
use crate::private_v8::Context;
use crate::private_v8::Local;
use crate::private_v8::MicrotaskQueue;
use crate::private_v8::Object;
use crate::private_v8::ObjectTemplate;
use crate::private_v8::Value;
use std::ptr::{null, null_mut};

unsafe extern "C" {
  fn v8__Context__New(
    isolate: *mut RealIsolate,
    templ: *const ObjectTemplate,
    global_object: *const Value,
    microtask_queue: *mut MicrotaskQueue,
  ) -> *const Context;
  fn v8__Context__Global(this: *const Context) -> *const Object;
}

#[derive(Default)]
pub struct ContextOptions<'s> {
  /// An optional object template from which the global object for the newly created context will
  /// be created.
  pub global_template: Option<Local<'s, ObjectTemplate>>,
  /// An optional global object to be reused for the newly created context. This global object
  /// must have been created by a previous call to Context::New with the same global template.
  /// The state of the global object will be completely reset and only object identify will
  /// remain.
  pub global_object: Option<Local<'s, Value>>,
  /// An optional microtask queue used to manage the microtasks created in this context. If not
  /// set the per-isolate default microtask queue would be used.
  pub microtask_queue: Option<*mut MicrotaskQueue>,
}

impl Context {
  /// Creates a new context.
  #[inline(always)]
  pub fn new<'s>(
    scope: &PinScope<'s, '_, ()>,
    options: ContextOptions,
  ) -> Local<'s, Context> {
    unsafe {
      scope.cast_local(|sd| {
        v8__Context__New(
          sd.get_isolate_ptr(),
          options
            .global_template
            .map_or_else(null, |t| &*t as *const _),
          options.global_object.map_or_else(null, |o| &*o as *const _),
          options.microtask_queue.unwrap_or_else(null_mut),
        )
      })
    }
    .unwrap()
  }


  /// Returns the global proxy object.
  ///
  /// Global proxy object is a thin wrapper whose prototype points to actual
  /// context's global object with the properties like Object, etc. This is done
  /// that way for security reasons (for more details see
  /// https://wiki.mozilla.org/Gecko:SplitWindow).
  ///
  /// Please note that changes to global proxy object prototype most probably
  /// would break VM---v8 expects only global object as a prototype of global
  /// proxy object.
  #[inline(always)]
  pub fn global<'s>(&self, scope: &PinScope<'s, '_, ()>) -> Local<'s, Object> {
    unsafe { scope.cast_local(|_| v8__Context__Global(self)) }.unwrap()
  }








}
