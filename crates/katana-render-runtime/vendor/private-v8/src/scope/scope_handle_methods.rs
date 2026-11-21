use super::{
  Context, HandleScope, Local, NewHandleScope,
  NonNull, PinnedRef, RealIsolate, ScopeData, ScopeStorage, raw,
};

impl ScopeData {
  #[inline(always)]
  pub(crate) fn get_isolate_ptr(&self) -> *mut RealIsolate {
    self.isolate.as_ptr()
  }

  pub(crate) fn get_current_context(&self) -> *mut Context {
    if let Some(context) = self.context.get() {
      context.as_ptr()
    } else {
      let isolate = self.get_isolate_ptr();
      let context =
        unsafe { raw::v8__Isolate__GetCurrentContext(isolate) }.cast_mut();
      self
        .context
        .set(Some(unsafe { NonNull::new_unchecked(context) }));
      context
    }
  }
}

impl<'s> HandleScope<'s> {
  #[allow(clippy::new_ret_no_self)]
  pub fn new<P: NewHandleScope<'s>>(
    scope: &'s mut P,
  ) -> ScopeStorage<P::NewScope> {
    ScopeStorage::new(P::make_new_scope(scope))
  }
}

impl<'s> PinnedRef<'s, HandleScope<'_>> {
  /// Returns the context of the currently running JavaScript, or the context
  /// on the top of the stack if no JavaScript is running
  pub fn get_current_context(&self) -> Local<'s, Context> {
    if let Some(context) = self.0.context.get() {
      unsafe { Local::from_non_null(context) }
    } else {
      let isolate = self.0.isolate;
      let context =
        unsafe { raw::v8__Isolate__GetCurrentContext(isolate.as_ptr()) }
          .cast_mut();
      unsafe {
        self.0.context.set(Some(NonNull::new_unchecked(context)));
        Local::from_raw_unchecked(context)
      }
    }
  }







}
