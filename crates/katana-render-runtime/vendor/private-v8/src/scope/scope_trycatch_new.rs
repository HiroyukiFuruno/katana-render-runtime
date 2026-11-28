use super::{
  CallbackScope,
  ClearCachedContext,
  ContextScope,
  EscapableHandleScope,
  GetIsolate,
  HandleScope,
  PhantomPinned,
  PinnedRef,
  Scope,
  TryCatch,
  raw,
};

pub trait NewTryCatch<'scope>: GetIsolate {
  type NewScope: Scope;
  fn make_new_scope(me: &'scope mut Self) -> Self::NewScope;
}

impl<'scope, 'obj: 'scope, 'i, C> NewTryCatch<'scope>
  for PinnedRef<'obj, HandleScope<'i, C>>
{
  type NewScope = TryCatch<'scope, 'obj, HandleScope<'i, C>>;
  fn make_new_scope(me: &'scope mut Self) -> Self::NewScope {
    TryCatch {
      scope: me,
      raw_try_catch: unsafe { raw::TryCatch::uninit() },
      _pinned: PhantomPinned,
    }
  }
}

/* the lifetimes here are: */
/* - 'borrow: the lifetime of the borrow of the scope */
/* - 'scope: the lifetime of the escapable handle scope */
/* - 'obj: the lifetime of the handles created from the escapable handle scope */
/* - 'esc: the lifetime of the slot that the escapable handle scope will eventually escape to. this must be longer than 'obj */
impl<'borrow, 'scope: 'borrow, 'obj: 'borrow, 'esc: 'obj, C>
  NewTryCatch<'borrow>
  for PinnedRef<'scope, EscapableHandleScope<'obj, 'esc, C>>
{
  type NewScope =
    TryCatch<'borrow, 'scope, EscapableHandleScope<'obj, 'esc, C>>;

  fn make_new_scope(me: &'borrow mut Self) -> Self::NewScope {
    TryCatch {
      scope: me,
      raw_try_catch: unsafe { raw::TryCatch::uninit() },
      _pinned: PhantomPinned,
    }
  }
}

impl<'scope, 'obj: 'scope, T: GetIsolate + Scope + ClearCachedContext>
  NewTryCatch<'scope> for ContextScope<'_, 'obj, T>
{
  type NewScope = TryCatch<'scope, 'obj, T>;
  fn make_new_scope(me: &'scope mut Self) -> Self::NewScope {
    TryCatch {
      scope: me,
      raw_try_catch: unsafe { raw::TryCatch::uninit() },
      _pinned: PhantomPinned,
    }
  }
}

impl<'scope, 'obj: 'scope, 'i, C> NewTryCatch<'scope>
  for PinnedRef<'obj, CallbackScope<'i, C>>
{
  type NewScope = TryCatch<'scope, 'i, HandleScope<'i, C>>;
  fn make_new_scope(me: &'scope mut Self) -> Self::NewScope {
    TryCatch {
      scope: unsafe {
        std::mem::transmute::<
          &mut PinnedRef<'_, CallbackScope<'_, C>>,
          &mut PinnedRef<'_, HandleScope<'_, C>>,
        >(me)
      },
      raw_try_catch: unsafe { raw::TryCatch::uninit() },
      _pinned: PhantomPinned,
    }
  }
}

impl<'scope, 'obj: 'scope, 'obj_outer: 'obj, 'iso, C> NewTryCatch<'scope>
  for PinnedRef<'obj, TryCatch<'_, 'obj_outer, HandleScope<'iso, C>>>
{
  type NewScope = TryCatch<'scope, 'obj_outer, HandleScope<'iso, C>>;
  fn make_new_scope(me: &'scope mut Self) -> Self::NewScope {
    TryCatch {
      scope: unsafe { me.as_mut_ref().0.get_unchecked_mut().scope },
      raw_try_catch: unsafe { raw::TryCatch::uninit() },
      _pinned: PhantomPinned,
    }
  }
}
