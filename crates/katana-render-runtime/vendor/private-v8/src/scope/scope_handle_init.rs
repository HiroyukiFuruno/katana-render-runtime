use super::{
  HandleScope,
  Pin,
  Scope,
  ScopeInit,
  ScopeStorage,
  raw,
  sealed,
};

impl<C> ScopeInit for HandleScope<'_, C> {
  fn init_stack(storage: Pin<&mut ScopeStorage<Self>>) -> Pin<&mut Self> {
    /* SAFETY: no moving the scope from this point on */
    let storage_mut = unsafe { storage.get_unchecked_mut() };
    unsafe {
      let isolate = storage_mut.scope.isolate;
      raw::HandleScope::init(&mut storage_mut.scope.raw_handle_scope, isolate)
    };

    let projected = &mut storage_mut.scope;

    /* SAFETY: scope is still pinned */
    unsafe { Pin::new_unchecked(projected) }
  }

  unsafe fn deinit(me: &mut Self) {
    unsafe { raw::v8__HandleScope__DESTRUCT(&mut me.raw_handle_scope) };
  }
}
impl<C> sealed::Sealed for HandleScope<'_, C> {}
impl<C> Scope for HandleScope<'_, C> {}
