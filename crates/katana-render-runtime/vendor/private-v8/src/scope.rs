#![doc = include_str!("scope/scope_module.md")]
use crate::private_v8::{
  Context, FunctionCallbackInfo, Isolate, Local,
  Message, Object, OwnedIsolate, PromiseRejectMessage,
  Value,
  isolate::{IsolateAnnex, RealIsolate},
  support::assert_layout_subset,
};
use std::{
  any::type_name,
  cell::Cell,
  marker::{PhantomData, PhantomPinned},
  mem::ManuallyDrop,
  ops::{Deref, DerefMut},
  pin::Pin,
  ptr::NonNull,
};
pub(crate) mod raw;

#[path = "scope/scope_types.rs"]
mod scope_types;
pub(crate) use scope_types::ScopeData;
pub use scope_types::{
  AllowJavascriptExecutionScope, CallbackScope, ContextScope,
  DisallowJavascriptExecutionScope, EscapableHandleScope, HandleScope,
  OnFailure, PinnedRef, ScopeStorage, TryCatch,
};
#[path = "scope/scope_traits.rs"]
mod scope_traits;
pub(crate) use scope_traits::GetIsolate;
use scope_traits::sealed;
use scope_traits::{ClearCachedContext, NewContextScope};
pub use scope_traits::{NewHandleScope, Scope, ScopeInit};
#[path = "scope/scope_callback_impls.rs"]
mod scope_callback_impls;
#[path = "scope/scope_context_cache.rs"]
mod scope_context_cache;
#[path = "scope/scope_context_impls.rs"]
mod scope_context_impls;
#[path = "scope/scope_get_isolate_impls.rs"]
mod scope_get_isolate_impls;
#[path = "scope/scope_handle_data.rs"]
mod scope_handle_data;
#[path = "scope/scope_handle_init.rs"]
mod scope_handle_init;
#[path = "scope/scope_handle_methods.rs"]
mod scope_handle_methods;
#[path = "scope/scope_new_handle_impls.rs"]
mod scope_new_handle_impls;
#[path = "scope/scope_storage_impls.rs"]
mod scope_storage_impls;
#[path = "scope/scope_trycatch_impls.rs"]
mod scope_trycatch_impls;
#[path = "scope/scope_trycatch_new.rs"]
mod scope_trycatch_new;
pub use scope_trycatch_new::NewTryCatch;
#[path = "scope/scope_escapable_impls.rs"]
mod scope_escapable_impls;
#[path = "scope/scope_execution_impls.rs"]
mod scope_execution_impls;
#[path = "scope/scope_pinned_access.rs"]
mod scope_pinned_access;
#[path = "scope/scope_pinned_deref.rs"]
mod scope_pinned_deref;

pub type PinScope<'s, 'i, C = Context> = PinnedRef<'s, HandleScope<'i, C>>;

#[path = "scope/scope_macros.rs"]
mod scope_macros;

#[cfg(test)]
pub(crate) use scope_macros::{
  allow_javascript_execution_scope, callback_scope, disallow_javascript_execution_scope,
  escapable_handle_scope,
};
pub(crate) use scope_macros::{scope, tc_scope};

#[cfg(test)]
mod tests {
  use super::*;
  use crate::private_v8::ContextOptions;
  use std::any::type_name;
  use std::pin::pin;

  trait SameType {}
  impl<A> SameType for (A, A) {}

  /// `AssertTypeOf` facilitates comparing types. The important difference with
  /// assigning a value to a variable with an explicitly stated type is that the
  /// latter allows coercions and dereferencing to change the type, whereas
  /// `AssertTypeOf` requires the compared types to match exactly.
  struct AssertTypeOf<'a, T>(#[allow(dead_code)] &'a T);
  impl<T> AssertTypeOf<'_, T> {
    pub fn is<A>(self)
    where
      (A, T): SameType,
    {
      assert_eq!(type_name::<A>(), type_name::<T>());
    }
  }

  #[path = "scope_deref_tests.rs"]
  mod deref_tests;
}
