use super::{
  Cell, ClearCachedContext, Context, IsolateAnnex, ManuallyDrop, NonNull,
  PhantomData, PhantomPinned, Pin, RealIsolate, ScopeInit,
  assert_layout_subset, raw,
};

/// Storage for a scope.
///
/// Tracks the initialization state of the scope, and holds the scope itself.
#[repr(C)]
pub struct ScopeStorage<T: ScopeInit> {
  pub(super) inited: bool,
  pub(super) scope: ManuallyDrop<T>,
  pub(super) _pinned: PhantomPinned,
}
/// A stack-allocated class that governs a number of local handles.
/// After a handle scope has been created, all local handles will be
/// allocated within that handle scope until either the handle scope is
/// deleted or another handle scope is created.  If there is already a
/// handle scope and a new one is created, all allocations will take
/// place in the new handle scope until it is deleted.  After that,
/// new handles will again be allocated in the original handle scope.
///
/// After the handle scope of a local handle has been deleted the
/// garbage collector will no longer track the object stored in the
/// handle and may deallocate it.  The behavior of accessing a handle
/// for which the handle scope has been deleted is undefined.
#[repr(C)]
#[derive(Debug)]
pub struct HandleScope<'s, C = Context> {
  pub(super) raw_handle_scope: raw::HandleScope,
  pub(super) isolate: NonNull<RealIsolate>,
  pub(super) annex: NonNull<IsolateAnnex>,
  pub(super) context: Cell<Option<NonNull<Context>>>,
  pub(super) _phantom: PhantomData<&'s mut C>,
  pub(super) _pinned: PhantomPinned,
}
#[repr(C)]
pub(crate) struct ScopeData {
  pub(super) isolate: NonNull<RealIsolate>,
  pub(super) context: Cell<Option<NonNull<Context>>>,
}

/// Stack-allocated class which sets the execution context for all operations
/// executed within a local scope. After entering a context, all code compiled
/// and run is compiled and run in this context.
#[repr(C)]
pub struct ContextScope<'borrow, 'scope, P: ClearCachedContext> {
  pub(super) raw_handle_scope: raw::ContextScope,
  pub(super) scope: &'borrow mut PinnedRef<'scope, P>,
}
/// A `CallbackScope` can be used to bootstrap a `HandleScope` and
/// `ContextScope` inside a callback function that gets called by V8.
/// Bootstrapping a scope inside a callback is the only valid use case of this
/// type; using it in other places leads to undefined behavior, which is also
/// the reason `CallbackScope::new()` is marked as being an unsafe function.
///
/// For some callback types, rusty_v8 internally creates a scope and passes it
/// as an argument to to embedder callback. Eventually we intend to wrap all
/// callbacks in this fashion, so the embedder would never needs to construct
/// a CallbackScope.
///
/// A `CallbackScope<()>`, without context, can be created from:
///   - `&mut Isolate`
///   - `&mut OwnedIsolate`
///
/// A `CallbackScope`, with context, can be created from:
///   - `Local<Context>`
///   - `Local<Message>`
///   - `Local<Object>`
///   - `Local<Promise>`
///   - `Local<SharedArrayBuffer>`
///   - `&FunctionCallbackInfo`
///   - `&PromiseRejectMessage`
#[repr(C)]
#[derive(Debug)]
pub struct CallbackScope<'s, C = Context> {
  pub(super) raw_handle_scope: raw::HandleScope,
  pub(super) isolate: NonNull<RealIsolate>,
  pub(super) annex: NonNull<IsolateAnnex>,
  pub(super) context: Cell<Option<NonNull<Context>>>,
  pub(super) _phantom: PhantomData<&'s C>,
  pub(super) _pinned: PhantomPinned,
}

assert_layout_subset!(HandleScope<'static, ()>, CallbackScope<'static, ()> { raw_handle_scope, isolate, annex, context, _phantom, _pinned });
const _: () = assert!(size_of::<HandleScope<'static, ()>>() == size_of::<CallbackScope<'static, ()>>());
/// An external exception handler.
#[repr(C)]
pub struct TryCatch<'scope, 'obj, P> {
  pub(super) raw_try_catch: raw::TryCatch,
  pub(super) scope: &'scope mut PinnedRef<'obj, P>,
  pub(super) _pinned: PhantomPinned,
}

/// A HandleScope which first allocates a handle in the current scope
/// which will be later filled with the escape value.
#[repr(C)]
pub struct EscapableHandleScope<'s, 'esc: 's, C = Context> {
  pub(super) raw_handle_scope: raw::HandleScope,
  pub(super) isolate: NonNull<RealIsolate>,
  pub(super) annex: NonNull<IsolateAnnex>,
  pub(super) context: Cell<Option<NonNull<Context>>>,
  pub(super) _phantom:
    PhantomData<(&'s mut raw::HandleScope, &'esc mut raw::EscapeSlot, &'s C)>,
  pub(super) _pinned: PhantomPinned,
  #[cfg(test)]
  pub(super) raw_escape_slot: Option<raw::EscapeSlot>,
}

assert_layout_subset!(HandleScope<'static, ()>, EscapableHandleScope<'static, 'static, ()> {
  raw_handle_scope,
  isolate,
  annex,
  context,
  _phantom,
  _pinned,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum OnFailure {
  CrashOnFailure,
  ThrowOnFailure,
  DumpOnFailure,
}

/* WHY: DisallowJavascriptExecutionScopeのC++列挙値と一致させる。 */
const _: () = {
  assert!(OnFailure::CrashOnFailure as std::ffi::c_int == 0);
  assert!(OnFailure::ThrowOnFailure as std::ffi::c_int == 1);
  assert!(OnFailure::DumpOnFailure as std::ffi::c_int == 2);
};

#[repr(C)]
pub struct DisallowJavascriptExecutionScope<'scope, 'obj, P> {
  pub(super) raw: raw::DisallowJavascriptExecutionScope,
  pub(super) scope: &'scope mut PinnedRef<'obj, P>,
  pub(super) on_failure: OnFailure,
  pub(super) _pinned: PhantomPinned,
}

#[repr(C)]
pub struct AllowJavascriptExecutionScope<'scope, 'obj, P> {
  pub(super) raw: raw::AllowJavascriptExecutionScope,
  pub(super) scope: &'scope mut PinnedRef<'obj, P>,
  pub(super) _pinned: PhantomPinned,
}

#[repr(transparent)]
pub struct PinnedRef<'p, T>(pub(super) Pin<&'p mut T>);
