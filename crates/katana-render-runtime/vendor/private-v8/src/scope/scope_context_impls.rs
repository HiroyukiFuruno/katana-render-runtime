use super::scope_pinned_deref::cast_pinned_ref_mut;
use super::{
  CallbackScope, ClearCachedContext, Context, ContextScope, Deref, DerefMut,
  EscapableHandleScope, GetIsolate, HandleScope, Local, NewContextScope, Pin,
  PinnedRef, Scope, ScopeInit, ScopeStorage, raw, sealed, type_name,
};

impl<P: ClearCachedContext> ScopeInit for ContextScope<'_, '_, P> {
  fn init_stack(storage: Pin<&mut ScopeStorage<Self>>) -> Pin<&mut Self> {
    storage.projected()
  }
  unsafe fn deinit(_me: &mut Self) {}
}

impl<'p, P: ClearCachedContext> Deref for ContextScope<'_, 'p, P> {
  type Target = PinnedRef<'p, P>;
  fn deref(&self) -> &Self::Target {
    self.scope
  }
}

impl<P: ClearCachedContext> DerefMut for ContextScope<'_, '_, P> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.scope
  }
}

impl<P: ClearCachedContext> sealed::Sealed for ContextScope<'_, '_, P> {}
impl<P: ClearCachedContext> Scope for ContextScope<'_, '_, P> {}
impl<P> Drop for ContextScope<'_, '_, P>
where
  P: ClearCachedContext,
{
  fn drop(&mut self) {
    self.scope.0.clear_cached_context();
  }
}

impl<'scope, 'obj: 'scope, 'ct, P: Scope + GetIsolate>
  NewContextScope<'scope, 'ct> for ContextScope<'_, 'obj, P>
where
  P: ClearCachedContext,
{
  type NewScope = ContextScope<'scope, 'obj, P>;

  fn make_new_scope(
    me: &'scope mut Self,
    context: Local<'ct, Context>,
  ) -> Self::NewScope {
    ContextScope {
      raw_handle_scope: raw::ContextScope::new(context),
      scope: me.scope,
    }
  }
}

impl<'scope, 'obj: 'scope, 'ct, 'i, C> NewContextScope<'scope, 'ct>
  for PinnedRef<'obj, HandleScope<'i, C>>
where
  'ct: 'scope,
{
  type NewScope = ContextScope<'scope, 'obj, HandleScope<'i>>;

  fn make_new_scope(
    me: &'scope mut Self,
    context: Local<'ct, Context>,
  ) -> Self::NewScope {
    me.0.context.set(None);
    ContextScope {
      raw_handle_scope: raw::ContextScope::new(context),
      scope: unsafe {
        /* SAFETY: we are adding the context, so we can mark that it now has a context. */
        /* the types are the same aside from the type parameter, which is only used in a ZST */
        cast_pinned_ref_mut::<HandleScope<'i, C>, HandleScope<'i, Context>>(me)
      },
    }
  }
}

impl<'scope, 'obj: 'scope, 'i, 'ct, C> NewContextScope<'scope, 'ct>
  for PinnedRef<'obj, CallbackScope<'i, C>>
{
  type NewScope = ContextScope<'scope, 'obj, HandleScope<'i>>;

  fn make_new_scope(
    me: &'scope mut Self,
    context: Local<'ct, Context>,
  ) -> Self::NewScope {
    ContextScope {
      raw_handle_scope: raw::ContextScope::new(context),
      scope: unsafe {
        /* we are adding the context, so we can mark that it now has a context. */
        /* SAFETY: CallbackScope is a superset of HandleScope, so giving a "view" of */
        /* the CallbackScope as a HandleScope is valid, and we won't ever move out of the transmuted */
        /* value */
        cast_pinned_ref_mut::<CallbackScope<'i, C>, HandleScope<'i, Context>>(
          me,
        )
      },
    }
  }
}

/* these lifetimes are crazy. basically we have */
/* - 'borrow: the borrow of the scope */
/* - 'scope: the lifetime of the scope. this must be longer than 'borrow. this is the lifetime of the handles created from the scope. */
/* - 'i: the lifetime of the inner the `EscapableHandleScope` is made from */
/* - 'esc: the lifetime of the slot that the `EscapableHandleScope` will eventually escape to. this must be longer than 'i */
/* - 'ct: the lifetime of the context (this is _not_ the same as 'scope, it can be longer or shorter) */
impl<'borrow, 'scope: 'borrow, 'i, 'esc: 'i, 'ct, C>
  NewContextScope<'borrow, 'ct>
  for PinnedRef<'scope, EscapableHandleScope<'i, 'esc, C>>
{
  type NewScope = ContextScope<'borrow, 'scope, EscapableHandleScope<'i, 'esc>>;

  fn make_new_scope(
    me: &'borrow mut Self,
    context: Local<'ct, Context>,
  ) -> Self::NewScope {
    ContextScope {
      raw_handle_scope: raw::ContextScope::new(context),
      scope: unsafe {
        /* SAFETY: layouts are the same aside from the type parameter, which is only used in a ZST */
        std::mem::transmute::<
          &'borrow mut PinnedRef<'scope, EscapableHandleScope<'i, 'esc, C>>,
          &'borrow mut PinnedRef<
            'scope,
            EscapableHandleScope<'i, 'esc, Context>,
          >,
        >(me)
      },
    }
  }
}

impl<P: ClearCachedContext> ClearCachedContext for ContextScope<'_, '_, P> {
  fn clear_cached_context(&self) {
    self.scope.0.clear_cached_context();
  }
}

impl<
  'scope,
  'obj: 'scope,
  'ct,
  P: NewContextScope<'scope, 'ct> + ClearCachedContext,
> ContextScope<'scope, 'obj, P>
{
  #[allow(clippy::new_ret_no_self)]
  pub fn new(
    param: &'scope mut P,
    context: Local<'ct, Context>,
  ) -> P::NewScope {
    if param.get_isolate_ptr() != unsafe { raw::v8__Isolate__GetCurrent() } {
      panic!(
        "{} and Context do not belong to the same Isolate",
        type_name::<P>()
      )
    }
    param.clear_cached_context();
    P::make_new_scope(param, context)
  }
}
