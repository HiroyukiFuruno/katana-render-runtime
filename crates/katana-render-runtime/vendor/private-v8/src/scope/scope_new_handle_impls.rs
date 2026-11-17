use super::{
  CallbackScope,
  Cell,
  ContextScope,
  GetIsolate,
  HandleScope,
  Isolate,
  NewHandleScope,
  NonNull,
  OwnedIsolate,
  PhantomData,
  PhantomPinned,
  PinnedRef,
  raw,
};

impl<'s, 'p: 's, C> NewHandleScope<'s> for PinnedRef<'_, HandleScope<'p, C>> {
  type NewScope = HandleScope<'s, C>;

  fn make_new_scope(me: &'s mut Self) -> Self::NewScope {
    HandleScope {
      raw_handle_scope: unsafe { raw::HandleScope::uninit() },
      isolate: me.0.isolate,
      annex: me.0.annex,
      context: Cell::new(me.0.context.get()),
      _phantom: PhantomData,
      _pinned: PhantomPinned,
    }
  }
}

impl<'s> NewHandleScope<'s> for Isolate {
  type NewScope = HandleScope<'s, ()>;

  #[inline(always)]
  fn make_new_scope(me: &'s mut Self) -> Self::NewScope {
    HandleScope {
      raw_handle_scope: unsafe { raw::HandleScope::uninit() },
      isolate: unsafe { NonNull::new_unchecked(me.as_real_ptr()) },
      annex: me.get_annex_ptr(),
      context: Cell::new(None),
      _phantom: PhantomData,
      _pinned: PhantomPinned,
    }
  }
}

impl<'s> NewHandleScope<'s> for OwnedIsolate {
  type NewScope = HandleScope<'s, ()>;

  fn make_new_scope(me: &'s mut Self) -> Self::NewScope {
    let isolate: &Isolate = me;
    HandleScope {
      raw_handle_scope: unsafe { raw::HandleScope::uninit() },
      isolate: unsafe { NonNull::new_unchecked(me.get_isolate_ptr()) },
      annex: isolate.get_annex_ptr(),
      context: Cell::new(None),
      _phantom: PhantomData,
      _pinned: PhantomPinned,
    }
  }
}

impl<'s, 'p: 's, 'i, C> NewHandleScope<'s>
  for PinnedRef<'p, CallbackScope<'i, C>>
{
  type NewScope = HandleScope<'i, C>;

  fn make_new_scope(me: &'s mut Self) -> Self::NewScope {
    HandleScope {
      raw_handle_scope: unsafe { raw::HandleScope::uninit() },
      isolate: me.0.isolate,
      annex: me.0.annex,
      context: Cell::new(me.0.context.get()),
      _phantom: PhantomData,
      _pinned: PhantomPinned,
    }
  }
}

impl<'a, 'i> NewHandleScope<'a> for ContextScope<'_, '_, HandleScope<'i>> {
  type NewScope = HandleScope<'i>;
  fn make_new_scope(me: &'a mut Self) -> Self::NewScope {
    HandleScope {
      raw_handle_scope: unsafe { raw::HandleScope::uninit() },
      isolate: unsafe { NonNull::new_unchecked(me.scope.get_isolate_ptr()) },
      annex: me.scope.0.annex,
      context: Cell::new(Some(me.raw_handle_scope.entered_context)),
      _phantom: PhantomData,
      _pinned: PhantomPinned,
    }
  }
}
