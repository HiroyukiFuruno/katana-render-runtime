use super::{
  Cell, GetIsolate, HandleScope, Local, Pin, PinnedRef, RealIsolate, ScopeData,
  Value, raw,
};

impl<'p> PinnedRef<'p, HandleScope<'_, ()>> {
  #[inline(always)]
  pub(crate) unsafe fn cast_local<T>(
    &self,
    _f: impl FnOnce(&mut ScopeData) -> *const T,
  ) -> Option<Local<'p, T>> {
    let mut data: ScopeData = ScopeData {
      context: Cell::new(self.0.context.get()),
      isolate: self.0.isolate,
    };
    let ptr = _f(&mut data);
    unsafe { Local::from_raw(ptr) }
  }

  /// Schedules an exception to be thrown when returning to JavaScript. When
  /// an exception has been scheduled it is illegal to invoke any
  /// JavaScript operation; the caller must return immediately and only
  /// after the exception has been handled does it become legal to invoke
  /// JavaScript operations.
  ///
  /// This function always returns the `undefined` value.
  pub fn throw_exception(
    &self,
    exception: Local<'p, Value>,
  ) -> Local<'p, Value> {
    unsafe {
      self.cast_local(|sd| {
        raw::v8__Isolate__ThrowException(sd.get_isolate_ptr(), &*exception)
      })
    }
    .unwrap()
  }

}

impl<C> GetIsolate for Pin<&mut HandleScope<'_, C>> {
  fn get_isolate_ptr(&self) -> *mut RealIsolate {
    self.isolate.as_ptr()
  }
}
