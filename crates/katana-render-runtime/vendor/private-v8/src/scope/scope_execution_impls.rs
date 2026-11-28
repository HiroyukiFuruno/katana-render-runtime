#[cfg(test)]
use super::{ClearCachedContext, ContextScope, OnFailure, PhantomPinned, PinnedRef};
use super::{
  AllowJavascriptExecutionScope,
  DisallowJavascriptExecutionScope,
  GetIsolate,
  NonNull,
  Pin,
  Scope,
  ScopeInit,
  ScopeStorage,
  raw,
  sealed,
};

impl<P: GetIsolate> ScopeInit for DisallowJavascriptExecutionScope<'_, '_, P> {
  fn init_stack(storage: Pin<&mut ScopeStorage<Self>>) -> Pin<&mut Self> {
    /* SAFETY: we aren't going to move the raw scope out */
    let storage_mut = unsafe { storage.get_unchecked_mut() };
    let isolate = storage_mut.scope.scope.get_isolate_ptr();
    let on_failure = storage_mut.scope.on_failure;
    /* SAFETY: calling the raw function, isolate is valid, and the raw scope won't be moved */
    unsafe {
      raw::DisallowJavascriptExecutionScope::init(
        &mut storage_mut.scope.raw,
        NonNull::new_unchecked(isolate),
        on_failure,
      );
      Pin::new_unchecked(&mut storage_mut.scope)
    }
  }

  unsafe fn deinit(me: &mut Self) {
    unsafe { raw::v8__DisallowJavascriptExecutionScope__DESTRUCT(&mut me.raw) };
  }
}

impl<P: GetIsolate> sealed::Sealed
  for DisallowJavascriptExecutionScope<'_, '_, P>
{
}
impl<P: Scope + GetIsolate> Scope
  for DisallowJavascriptExecutionScope<'_, '_, P>
{
}

#[cfg(test)]
impl<'scope, P: NewDisallowJavascriptExecutionScope<'scope>>
  DisallowJavascriptExecutionScope<'scope, '_, P>
{
  #[allow(clippy::new_ret_no_self)]
  pub fn new(
    param: &'scope mut P,
    on_failure: OnFailure,
  ) -> ScopeStorage<P::NewScope> {
    ScopeStorage::new(P::make_new_scope(param, on_failure))
  }
}

#[cfg(test)]
pub trait NewDisallowJavascriptExecutionScope<'scope> {
  type NewScope: Scope;
  fn make_new_scope(
    me: &'scope mut Self,
    on_failure: OnFailure,
  ) -> Self::NewScope;
}

#[cfg(test)]
impl<'scope, 'obj, P> NewDisallowJavascriptExecutionScope<'scope>
  for ContextScope<'_, 'obj, P>
where
  P: ClearCachedContext,
  PinnedRef<'obj, P>: NewDisallowJavascriptExecutionScope<'scope>,
{
  type NewScope = <PinnedRef<'obj, P> as NewDisallowJavascriptExecutionScope<
    'scope,
  >>::NewScope;
  fn make_new_scope(
    me: &'scope mut Self,
    on_failure: OnFailure,
  ) -> Self::NewScope {
    PinnedRef::<'obj, P>::make_new_scope(me.scope, on_failure)
  }
}

#[cfg(test)]
impl<'scope, 'obj: 'scope, P: Scope + GetIsolate>
  NewDisallowJavascriptExecutionScope<'scope> for PinnedRef<'obj, P>
{
  type NewScope = DisallowJavascriptExecutionScope<'scope, 'obj, P>;

  fn make_new_scope(
    me: &'scope mut Self,
    on_failure: OnFailure,
  ) -> Self::NewScope {
    DisallowJavascriptExecutionScope {
      raw: unsafe { raw::DisallowJavascriptExecutionScope::uninit() },
      scope: me,
      on_failure,
      _pinned: PhantomPinned,
    }
  }
}
impl<P: GetIsolate> ScopeInit for AllowJavascriptExecutionScope<'_, '_, P> {
  fn init_stack(storage: Pin<&mut ScopeStorage<Self>>) -> Pin<&mut Self> {
    let storage_mut = unsafe { storage.get_unchecked_mut() };
    let isolate = unsafe {
      NonNull::new_unchecked(storage_mut.scope.scope.get_isolate_ptr())
    };
    unsafe {
      raw::AllowJavascriptExecutionScope::init(
        &mut storage_mut.scope.raw,
        isolate,
      );
    }
    let projected = &mut storage_mut.scope;
    unsafe { Pin::new_unchecked(projected) }
  }

  unsafe fn deinit(me: &mut Self) {
    unsafe { raw::v8__AllowJavascriptExecutionScope__DESTRUCT(&mut me.raw) };
  }
}

impl<P: GetIsolate> sealed::Sealed
  for AllowJavascriptExecutionScope<'_, '_, P>
{
}
impl<P: Scope + GetIsolate> Scope for AllowJavascriptExecutionScope<'_, '_, P> {}

#[cfg(test)]
impl<'scope, P: NewAllowJavascriptExecutionScope<'scope>>
  AllowJavascriptExecutionScope<'scope, '_, P>
{
  #[allow(clippy::new_ret_no_self)]
  pub fn new(param: &'scope mut P) -> ScopeStorage<P::NewScope> {
    ScopeStorage::new(P::make_new_scope(param))
  }
}

#[cfg(test)]
pub trait NewAllowJavascriptExecutionScope<'scope> {
  type NewScope: Scope;
  fn make_new_scope(me: &'scope mut Self) -> Self::NewScope;
}

#[cfg(test)]
impl<'scope, 'obj: 'scope, P: Scope + GetIsolate>
  NewAllowJavascriptExecutionScope<'scope> for PinnedRef<'obj, P>
{
  type NewScope = AllowJavascriptExecutionScope<'scope, 'obj, P>;
  fn make_new_scope(me: &'scope mut Self) -> Self::NewScope {
    AllowJavascriptExecutionScope {
      raw: unsafe { raw::AllowJavascriptExecutionScope::uninit() },
      scope: me,
      _pinned: PhantomPinned,
    }
  }
}
