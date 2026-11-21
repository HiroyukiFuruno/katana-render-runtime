#[cfg(test)]
use super::{Cell, ContextScope, Isolate, Local, NonNull, PhantomData, PhantomPinned};
use super::{
  Deref,
  DerefMut,
  EscapableHandleScope,
  HandleScope,
  Pin,
  PinnedRef,
  Scope,
  ScopeInit,
  ScopeStorage,
  raw,
  sealed,
};

impl<'s, 'esc: 's, C> ScopeInit for EscapableHandleScope<'s, 'esc, C> {
  fn init_stack(storage: Pin<&mut ScopeStorage<Self>>) -> Pin<&mut Self> {
    let storage_mut = unsafe { storage.get_unchecked_mut() };
    unsafe {
      let isolate = storage_mut.scope.isolate;
      raw::HandleScope::init(&mut storage_mut.scope.raw_handle_scope, isolate);
    }
    let projected = &mut storage_mut.scope;

    unsafe { Pin::new_unchecked(projected) }
  }

  unsafe fn deinit(me: &mut Self) {
    unsafe { raw::v8__HandleScope__DESTRUCT(&raw mut me.raw_handle_scope) };
  }
}

impl<'s, 'esc: 's> EscapableHandleScope<'s, 'esc> {
  #[cfg(test)]
  #[allow(clippy::new_ret_no_self)]
  pub fn new<P: NewEscapableHandleScope<'s>>(
    scope: &'s mut P,
  ) -> ScopeStorage<P::NewScope> {
    ScopeStorage::new(P::make_new_scope(scope))
  }
}

impl<'s, 'esc: 's, C> PinnedRef<'_, EscapableHandleScope<'s, 'esc, C>> {
  /// Pushes the value into the previous scope and returns a handle to it.
  /// Cannot be called twice.
  #[cfg(test)]
  #[allow(clippy::expect_used)]
  pub fn escape<'a, T>(&mut self, value: Local<'a, T>) -> Local<'esc, T>
  where
    for<'l> Local<'l, T>: Into<Local<'l, crate::private_v8::Data>>,
  {
    let escape_slot = unsafe { self.0.as_mut().get_unchecked_mut() }
      .raw_escape_slot
      .take()
      .expect("EscapableHandleScope::escape() called twice");
    escape_slot.escape(value)
  }
}

impl<'p, 's, 'esc: 's, C> Deref
  for PinnedRef<'p, EscapableHandleScope<'s, 'esc, C>>
{
  type Target = PinnedRef<'p, HandleScope<'s, C>>;
  fn deref(&self) -> &Self::Target {
    unsafe { std::mem::transmute(self) }
  }
}

impl<'s, 'esc: 's, C> DerefMut
  for PinnedRef<'_, EscapableHandleScope<'s, 'esc, C>>
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { std::mem::transmute(self) }
  }
}

#[cfg(test)]
pub trait NewEscapableHandleScope<'s> {
  type NewScope: Scope;
  fn make_new_scope(me: &'s mut Self) -> Self::NewScope;
}

#[cfg(test)]
impl<'s> NewEscapableHandleScope<'s> for Isolate {
  type NewScope = EscapableHandleScope<'s, 's, ()>;

  fn make_new_scope(me: &'s mut Self) -> Self::NewScope {
    let isolate = unsafe { NonNull::new_unchecked(me.as_real_ptr()) };
    let annex = me.get_annex_ptr();
    let raw_escape_slot = raw::EscapeSlot::new(isolate);
    let raw_handle_scope = unsafe { raw::HandleScope::uninit() };

    EscapableHandleScope {
      isolate,
      annex,
      context: Cell::new(None),
      raw_escape_slot: Some(raw_escape_slot),
      raw_handle_scope,
      _phantom: PhantomData,
      _pinned: PhantomPinned,
    }
  }
}

#[cfg(test)]
impl<'s, 'obj: 's, C> NewEscapableHandleScope<'s>
  for PinnedRef<'obj, HandleScope<'_, C>>
{
  type NewScope = EscapableHandleScope<'s, 'obj, C>;
  fn make_new_scope(me: &'s mut Self) -> Self::NewScope {
    /* Note: the `raw_escape_slot` field must be initialized _before_ the */
    /* `raw_handle_scope` field, otherwise the escaped local handle ends up */
    /* inside the `EscapableHandleScope` that's being constructed here, */
    /* rather than escaping from it. */
    let isolate = me.0.isolate;
    let annex = me.0.annex;
    let raw_escape_slot = raw::EscapeSlot::new(isolate);
    let raw_handle_scope = unsafe { raw::HandleScope::uninit() };

    EscapableHandleScope {
      isolate,
      annex,
      context: Cell::new(me.0.context.get()),
      raw_escape_slot: Some(raw_escape_slot),
      raw_handle_scope,
      _phantom: PhantomData,
      _pinned: PhantomPinned,
    }
  }
}

#[cfg(test)]
impl<'borrow, 'obj: 'borrow, C> NewEscapableHandleScope<'borrow>
  for ContextScope<'_, 'obj, HandleScope<'_, C>>
{
  type NewScope = EscapableHandleScope<'borrow, 'obj, C>;
  fn make_new_scope(me: &'borrow mut Self) -> Self::NewScope {
    NewEscapableHandleScope::make_new_scope(me.scope)
  }
}

#[cfg(test)]
impl<'borrow, 's: 'borrow, 'esc: 'borrow, C> NewEscapableHandleScope<'borrow>
  for PinnedRef<'_, EscapableHandleScope<'s, 'esc, C>>
{
  type NewScope = EscapableHandleScope<'borrow, 's, C>;
  fn make_new_scope(me: &'borrow mut Self) -> Self::NewScope {
    /* Note: the `raw_escape_slot` field must be initialized _before_ the */
    /* `raw_handle_scope` field, otherwise the escaped local handle ends up */
    /* inside the `EscapableHandleScope` that's being constructed here, */
    /* rather than escaping from it. */
    let isolate = me.0.isolate;
    let annex = me.0.annex;
    let raw_escape_slot = raw::EscapeSlot::new(isolate);
    let raw_handle_scope = unsafe { raw::HandleScope::uninit() };
    EscapableHandleScope {
      isolate,
      annex,
      context: Cell::new(me.0.context.get()),
      raw_escape_slot: Some(raw_escape_slot),
      raw_handle_scope,
      _phantom: PhantomData,
      _pinned: PhantomPinned,
    }
  }
}

impl<'s, 'esc: 's, C> sealed::Sealed for EscapableHandleScope<'s, 'esc, C> {}
impl<'s, 'esc: 's, C> Scope for EscapableHandleScope<'s, 'esc, C> {}
