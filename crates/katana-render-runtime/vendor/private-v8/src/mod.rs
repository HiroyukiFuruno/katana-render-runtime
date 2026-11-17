/* Copyright 2019-2021 the Deno authors. All rights reserved. MIT license. */

//! # Example
//!
//! ```rust
//! let platform = v8::Platform::new(0, false).make_shared();
//! v8::V8::initialize_platform(platform);
//! v8::V8::initialize();
//!
//! let isolate = &mut v8::Isolate::new(Default::default());
//!
//! let scope = std::pin::pin!(v8::HandleScope::new(isolate));
//! let scope = &mut scope.init();
//! let context = v8::Context::new(scope, Default::default());
//! let scope = &mut v8::ContextScope::new(scope, context);
//!
//! let code = v8::String::new(scope, "'Hello' + ' World!'").unwrap();
//! println!("javascript code: {}", code.to_rust_string_lossy(scope));
//!
//! let script = v8::Script::compile(scope, code, None).unwrap();
//! let result = script.run(scope).unwrap();
//! let result = result.to_string(scope).unwrap();
//! println!("result: {}", result.to_rust_string_lossy(scope));
//! ```

#![allow(clippy::missing_safety_doc)]
/* WHY: 上流の修正版が公開されるまで、V8内部のコードスタイルと異常時の挙動を維持する。 */
#![allow(
  clippy::empty_line_after_doc_comments,
  clippy::enum_variant_names,
  clippy::panic,
  clippy::unwrap_used,
  clippy::wildcard_imports,
  clippy::wrong_self_convention
)]

mod array_buffer;
mod binding;
mod context;
#[cfg(test)]
pub use context::ContextOptions;
pub mod cppgc;
mod data;
mod exception;
mod function;
mod handle;
mod isolate;
mod isolate_create_params;
mod microtask;
mod module;
mod name;
mod number;
mod object;
mod platform;
mod primitives;
mod promise;
mod private;
mod property_attribute;
mod scope;
mod script;
mod string;
mod support;
mod value;
mod value_deserializer;
mod value_serializer;

pub mod crdtp;
pub mod inspector;
pub mod simdutf;
/* This module is intentionally named "V8" rather than "v8" to match the */
/* C++ namespace "v8::V8". */
#[allow(non_snake_case)]
#[path = "V8.rs"]
mod v8_platform;
pub use v8_platform::V8;

pub use data::*;
pub use exception::*;
pub use function::*;
pub use handle::{Global, Local};
pub use isolate::{
  Isolate, IsolateHandle, OwnedIsolate,
  RealIsolate,
};
pub use microtask::MicrotaskQueue;
pub use platform::Platform;
pub use promise::{PromiseRejectMessage, PromiseState};
pub use property_attribute::*;
pub use scope::{
  CallbackScope, ContextScope, HandleScope, OnFailure, PinScope, PinnedRef,
  TryCatch,
};
pub use script::ScriptOrigin;

/* TODO(piscisaureus): Ideally this trait would not be exported. */
pub use support::MapFnTo;

#[cfg(test)]
pub(crate) fn initialize_v8() {
  crate::markdown::diagram_js_runtime::DiagramV8Runtime::ensure_initialized();
}

#[cfg(test)]
mod liveness_tests;

#[cfg(test)]
pub(crate) use self::object::IntegrityLevel;
#[cfg(test)]
pub(crate) use self::scope::{
  AllowJavascriptExecutionScope, DisallowJavascriptExecutionScope, EscapableHandleScope,
};
#[cfg(test)]
pub(crate) use self::scope::{
  allow_javascript_execution_scope, callback_scope,
  disallow_javascript_execution_scope, escapable_handle_scope,
};
pub(crate) use self::scope::{scope, tc_scope};
