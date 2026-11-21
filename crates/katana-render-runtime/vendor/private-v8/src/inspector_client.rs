use super::*;
use crate::private_v8::PinScope;

#[repr(C)]
#[derive(Debug)]
pub struct RawV8InspectorClient {
  _cxx_vtable: CxxVTable,
}

#[allow(unused_variables)]
pub trait V8InspectorClientImpl {
  fn run_message_loop_on_pause(&self, context_group_id: i32) {}
  fn quit_message_loop_on_pause(&self) {}
  fn run_if_waiting_for_debugger(&self, context_group_id: i32) {}

  fn generate_unique_id(&self) -> i64 {
    0 // 0 = let V8 pick a unique id itself
  }

  #[allow(clippy::too_many_arguments)]
  fn console_api_message(
    &self,
    context_group_id: i32,
    level: i32,
    message: &StringView,
    url: &StringView,
    line_number: u32,
    column_number: u32,
    stack_trace: &mut V8StackTrace,
  ) {
  }

  /// Returns a custom Chrome DevTools Protocol `Runtime.RemoteObject` subtype
  /// for `value`. Use one of the protocol's defined subtype enum values.
  /// Returning `Some` causes V8 to call
  /// [`Self::description_for_value_subtype`].
  ///
  /// The callback scope uses the isolate's current context as a best-effort
  /// context; it is not necessarily the context in which `value` originated.
  /// If the isolate has no current context, V8 skips this callback entirely.
  /// This callback runs while the inspector is constructing a value mirror.
  /// Operations such as property access can execute JavaScript through getters
  /// or proxies, so wrap them in a [`crate::private_v8::TryCatch`] to avoid leaving a
  /// pending exception in the inspector's mirror-building path.
  fn value_subtype<'s>(
    &self,
    scope: &mut PinScope<'s, '_>,
    value: Local<'s, Value>,
  ) -> Option<UniquePtr<StringBuffer>> {
    None
  }

  /// Returns the description for a value whose custom subtype was returned by
  /// [`Self::value_subtype`]. Returning `None` makes V8 fall back to the default
  /// object mirror, which also discards the custom subtype unless it is
  /// `"error"` or `"array"`.
  ///
  /// Like [`Self::value_subtype`], this callback runs while the inspector is
  /// constructing a value mirror. Wrap operations that can execute JavaScript
  /// in a [`crate::private_v8::TryCatch`] so an exception does not remain pending.
  fn description_for_value_subtype<'s>(
    &self,
    scope: &mut PinScope<'s, '_>,
    value: Local<'s, Value>,
  ) -> Option<UniquePtr<StringBuffer>> {
    None
  }

  fn ensure_default_context_in_group(
    &self,
    context_group_id: i32,
  ) -> Option<Local<'_, Context>> {
    None
  }

  fn resource_name_to_url(
    &self,
    resource_name: &StringView,
  ) -> Option<UniquePtr<StringBuffer>> {
    None
  }
}

/* V8 will hold onto a raw pointer to the RawV8InspectorClient, so we need to */
/* make sure it stays pinned. */
#[repr(C)]
pub(super) struct V8InspectorClientHeap {
  raw: UnsafeCell<RawV8InspectorClient>,
  /* this doesn't need to be pinned, but it's convenient to keep it here */
  /* so we can access it from a pointer to the RawV8InspectorClient */
  pub(super) imp: Box<dyn V8InspectorClientImpl>,
  _pinned: PhantomPinned,
}

impl V8InspectorClientHeap {
  pub(super) unsafe fn from_raw<'b>(
    this: *const RawV8InspectorClient,
  ) -> &'b V8InspectorClientHeap {
    unsafe { &(*this.cast::<V8InspectorClientHeap>()) }
  }
}
