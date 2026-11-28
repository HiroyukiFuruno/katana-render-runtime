use super::scope_pinned_deref::cast_pinned_ref;
use super::{
  AllowJavascriptExecutionScope, CallbackScope, ContextScope,
  DisallowJavascriptExecutionScope, EscapableHandleScope, HandleScope, Isolate,
  PinnedRef, TryCatch,
};

impl<C> AsRef<Isolate> for PinnedRef<'_, HandleScope<'_, C>> {
  fn as_ref(&self) -> &Isolate {
    unsafe { Isolate::from_raw_ref(&self.0.isolate) }
  }
}

impl<C> AsMut<Isolate> for PinnedRef<'_, HandleScope<'_, C>> {
  fn as_mut(&mut self) -> &mut Isolate {
    unsafe {
      Isolate::from_raw_ref_mut(
        &mut self.0.as_mut().get_unchecked_mut().isolate,
      )
    }
  }
}

/* Cached slot accessors that use the annex pointer stored in the scope, */
/* avoiding an FFI call to v8__Isolate__GetData on every access. */
/* These shadow the Isolate::get_slot/set_slot/etc. methods in the Deref chain. */
impl<C> PinnedRef<'_, HandleScope<'_, C>> {
  /// Get a reference to embedder data added with `set_slot()`.
  #[inline(always)]
  pub fn get_slot<T: 'static>(&self) -> Option<&T> {
    let annex = unsafe { self.0.annex.as_ref() };
    annex.get_slot::<T>()
  }

}

impl<C> AsRef<Isolate> for PinnedRef<'_, CallbackScope<'_, C>> {
  fn as_ref(&self) -> &Isolate {
    unsafe { Isolate::from_raw_ref(&self.0.isolate) }
  }
}

impl<C> AsMut<Isolate> for PinnedRef<'_, CallbackScope<'_, C>> {
  fn as_mut(&mut self) -> &mut Isolate {
    unsafe {
      Isolate::from_raw_ref_mut(
        &mut self.0.as_mut().get_unchecked_mut().isolate,
      )
    }
  }
}

impl<C> AsRef<Isolate> for PinnedRef<'_, TryCatch<'_, '_, HandleScope<'_, C>>> {
  fn as_ref(&self) -> &Isolate {
    unsafe { Isolate::from_raw_ref(&self.0.scope.0.isolate) }
  }
}

impl<C> AsMut<Isolate> for PinnedRef<'_, TryCatch<'_, '_, HandleScope<'_, C>>> {
  fn as_mut(&mut self) -> &mut Isolate {
    unsafe {
      Isolate::from_raw_ref_mut(
        &mut self
          .0
          .as_mut()
          .get_unchecked_mut()
          .scope
          .0
          .as_mut()
          .get_unchecked_mut()
          .isolate,
      )
    }
  }
}

impl<C> AsRef<Isolate>
  for PinnedRef<
    '_,
    DisallowJavascriptExecutionScope<'_, '_, HandleScope<'_, C>>,
  >
{
  fn as_ref(&self) -> &Isolate {
    unsafe { Isolate::from_raw_ref(&self.0.scope.0.isolate) }
  }
}

impl<C> AsRef<Isolate>
  for PinnedRef<'_, AllowJavascriptExecutionScope<'_, '_, HandleScope<'_, C>>>
{
  fn as_ref(&self) -> &Isolate {
    unsafe { Isolate::from_raw_ref(&self.0.scope.0.isolate) }
  }
}

impl AsRef<Isolate> for PinnedRef<'_, EscapableHandleScope<'_, '_>> {
  fn as_ref(&self) -> &Isolate {
    unsafe { Isolate::from_raw_ref(&self.0.isolate) }
  }
}

impl<C> AsRef<Isolate> for ContextScope<'_, '_, HandleScope<'_, C>> {
  fn as_ref(&self) -> &Isolate {
    unsafe { Isolate::from_raw_ref(&self.scope.0.isolate) }
  }
}
impl<C> AsRef<Isolate> for ContextScope<'_, '_, CallbackScope<'_, C>> {
  fn as_ref(&self) -> &Isolate {
    unsafe { Isolate::from_raw_ref(&self.scope.0.isolate) }
  }
}

impl<'pin, 's, 'esc: 's, C> AsRef<PinnedRef<'pin, HandleScope<'s, C>>>
  for PinnedRef<'pin, EscapableHandleScope<'s, 'esc, C>>
{
  fn as_ref(&self) -> &PinnedRef<'pin, HandleScope<'s, C>> {
    unsafe {
      cast_pinned_ref::<EscapableHandleScope<'s, 'esc, C>, HandleScope<'s, C>>(
        self,
      )
    }
  }
}

impl<'obj, 'inner, C> AsRef<PinnedRef<'obj, HandleScope<'inner, C>>>
  for PinnedRef<'obj, HandleScope<'inner, C>>
{
  fn as_ref(&self) -> &PinnedRef<'obj, HandleScope<'inner, C>> {
    self
  }
}
