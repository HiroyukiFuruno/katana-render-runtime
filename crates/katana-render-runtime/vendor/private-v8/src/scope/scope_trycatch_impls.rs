use super::{
  GetIsolate,
  HandleScope,
  Local,
  Message,
  NewTryCatch,
  NonNull,
  Pin,
  PinnedRef,
  Scope,
  ScopeInit,
  ScopeStorage,
  TryCatch,
  Value,
  raw,
  sealed,
};

impl<'scope, P: NewTryCatch<'scope>> TryCatch<'scope, '_, P> {
  #[allow(clippy::new_ret_no_self)]
  pub fn new(param: &'scope mut P) -> ScopeStorage<P::NewScope> {
    ScopeStorage::new(P::make_new_scope(param))
  }
}

impl<P: GetIsolate> ScopeInit for TryCatch<'_, '_, P> {
  fn init_stack(storage: Pin<&mut ScopeStorage<Self>>) -> Pin<&mut Self> {
    let storage_mut = unsafe { storage.get_unchecked_mut() };
    let isolate = unsafe {
      NonNull::new_unchecked(storage_mut.scope.scope.get_isolate_ptr())
    };
    unsafe {
      raw::TryCatch::init(&mut storage_mut.scope.raw_try_catch, isolate);
    }
    let projected = &mut storage_mut.scope;
    unsafe { Pin::new_unchecked(projected) }
  }

  unsafe fn deinit(me: &mut Self) {
    unsafe { raw::v8__TryCatch__DESTRUCT(&mut me.raw_try_catch) };
  }
}

impl<'scope, 'obj: 'scope, 'iso: 'obj, P: GetIsolate>
  PinnedRef<'_, TryCatch<'scope, 'obj, P>>
where
  PinnedRef<'obj, P>: AsRef<PinnedRef<'obj, HandleScope<'iso>>>,
{
  /// Returns true if an exception has been caught by this try/catch block.
  #[inline(always)]
  #[cfg(test)]
  pub fn has_caught(&self) -> bool {
    unsafe { raw::v8__TryCatch__HasCaught(self.get_raw()) }
  }


  /// Returns true if an exception has been caught due to script execution
  /// being terminated.
  ///
  /// There is no JavaScript representation of an execution termination
  /// exception. Such exceptions are thrown when the TerminateExecution
  /// methods are called to terminate a long-running script.
  ///
  /// If such an exception has been thrown, HasTerminated will return true,
  /// indicating that it is possible to call CancelTerminateExecution in order
  /// to continue calling into the engine.
  #[inline(always)]
  pub fn has_terminated(&self) -> bool {
    unsafe { raw::v8__TryCatch__HasTerminated(self.get_raw()) }
  }




  /// Clears any exceptions that may have been caught by this try/catch block.
  /// After this method has been called, HasCaught() will return false. Cancels
  /// the scheduled exception if it is caught and ReThrow() is not called
  /// before.
  ///
  /// It is not necessary to clear a try/catch block before using it again; if
  /// another exception is thrown the previously caught exception will just be
  /// overwritten. However, it is often a good idea since it makes it easier
  /// to determine which operation threw a given exception.
  #[inline(always)]
  #[cfg(test)]
  pub fn reset(&mut self) {
    unsafe { raw::v8__TryCatch__Reset(self.get_raw_mut()) };
  }

  #[inline(always)]
  fn get_raw(&self) -> &raw::TryCatch {
    &self.0.raw_try_catch
  }

  #[inline(always)]
  #[cfg(test)]
  unsafe fn get_raw_mut(&mut self) -> &mut raw::TryCatch {
    unsafe { &mut self.0.as_mut().get_unchecked_mut().raw_try_catch }
  }

  pub fn exception(&self) -> Option<Local<'obj, Value>> {
    unsafe {
      self
        .0
        .scope
        .as_ref()
        .cast_local(|_data| raw::v8__TryCatch__Exception(self.get_raw()))
    }
  }

  pub fn message(&self) -> Option<Local<'obj, Message>> {
    unsafe {
      self
        .0
        .scope
        .as_ref()
        .cast_local(|_data| raw::v8__TryCatch__Message(self.get_raw()))
    }
  }


  pub fn stack_trace(&self) -> Option<Local<'obj, Value>> {
    unsafe {
      self.0.scope.as_ref().cast_local(|_data| {
        raw::v8__TryCatch__StackTrace(
          self.get_raw(),
          _data.get_current_context(),
        )
      })
    }
  }
}

impl<P> sealed::Sealed for TryCatch<'_, '_, P> {}
impl<P: Scope + GetIsolate> Scope for TryCatch<'_, '_, P> {}
