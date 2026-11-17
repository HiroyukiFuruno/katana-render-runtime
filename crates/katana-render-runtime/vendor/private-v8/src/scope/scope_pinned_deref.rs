use super::{
  AllowJavascriptExecutionScope, CallbackScope, Deref, DerefMut,
  DisallowJavascriptExecutionScope, EscapableHandleScope, GetIsolate,
  HandleScope, Isolate, Pin, PinnedRef, RealIsolate, TryCatch,
};

impl<'p, T> From<Pin<&'p mut T>> for PinnedRef<'p, T> {
  fn from(value: Pin<&'p mut T>) -> Self {
    PinnedRef(value)
  }
}

impl<T> PinnedRef<'_, T> {
  pub fn as_mut_ref(&mut self) -> PinnedRef<'_, T> {
    PinnedRef(self.0.as_mut())
  }
}

pub(super) unsafe fn cast_pinned_ref<'b, 'o, I, O>(
  pinned: &PinnedRef<'_, I>,
) -> &'b PinnedRef<'o, O> {
  unsafe { std::mem::transmute(pinned) }
}

pub(super) unsafe fn cast_pinned_ref_mut<'b, 'o, I, O>(
  pinned: &mut PinnedRef<'_, I>,
) -> &'b mut PinnedRef<'o, O> {
  unsafe { std::mem::transmute(pinned) }
}

impl<'p, 'i> Deref for PinnedRef<'p, HandleScope<'i>> {
  type Target = PinnedRef<'p, HandleScope<'i, ()>>;
  fn deref(&self) -> &Self::Target {
    unsafe { cast_pinned_ref::<HandleScope<'i>, HandleScope<'i, ()>>(self) }
  }
}

impl DerefMut for PinnedRef<'_, HandleScope<'_>> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { cast_pinned_ref_mut::<HandleScope<'_>, HandleScope<'_, ()>>(self) }
  }
}

impl Deref for PinnedRef<'_, HandleScope<'_, ()>> {
  type Target = Isolate;
  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    unsafe { Isolate::from_raw_ref(&self.0.isolate) }
  }
}

impl DerefMut for PinnedRef<'_, HandleScope<'_, ()>> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe {
      Isolate::from_raw_ref_mut(
        &mut self.0.as_mut().get_unchecked_mut().isolate,
      )
    }
  }
}

impl<'i> Deref for PinnedRef<'_, CallbackScope<'i>> {
  /* You may notice the output lifetime is a little bit weird here. */
  /* Basically, we're saying that any Handles created from this `CallbackScope` */
  /* will live as long as the thing that we made the `CallbackScope` from. */
  /* In practice, this means that the caller of `CallbackScope::new` needs to */
  /* be careful to ensure that the input lifetime is a safe approximation. */
  type Target = PinnedRef<'i, HandleScope<'i>>;
  fn deref(&self) -> &Self::Target {
    unsafe { cast_pinned_ref::<CallbackScope<'i>, HandleScope<'i>>(self) }
  }
}

impl DerefMut for PinnedRef<'_, CallbackScope<'_>> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { cast_pinned_ref_mut::<CallbackScope<'_>, HandleScope<'_>>(self) }
  }
}

impl<'i> Deref for PinnedRef<'_, CallbackScope<'i, ()>> {
  type Target = PinnedRef<'i, HandleScope<'i, ()>>;
  fn deref(&self) -> &Self::Target {
    unsafe {
      cast_pinned_ref::<CallbackScope<'i, ()>, HandleScope<'i, ()>>(self)
    }
  }
}

impl DerefMut for PinnedRef<'_, CallbackScope<'_, ()>> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe {
      cast_pinned_ref_mut::<CallbackScope<'_, ()>, HandleScope<'_, ()>>(self)
    }
  }
}

impl<'obj, 'iso, C> Deref
  for PinnedRef<'_, TryCatch<'_, 'obj, HandleScope<'iso, C>>>
{
  type Target = PinnedRef<'obj, HandleScope<'iso, C>>;
  fn deref(&self) -> &Self::Target {
    self.0.scope
  }
}

impl<C> DerefMut for PinnedRef<'_, TryCatch<'_, '_, HandleScope<'_, C>>> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    /* SAFETY: we're just projecting the pinned reference, it still can't be moved */
    unsafe { self.as_mut_ref().0.get_unchecked_mut().scope }
  }
}

impl<'borrow, 'scope: 'borrow, 'obj: 'borrow, 'esc: 'obj, C> Deref
  for PinnedRef<
    '_,
    TryCatch<'borrow, 'scope, EscapableHandleScope<'obj, 'esc, C>>,
  >
{
  type Target = PinnedRef<'scope, EscapableHandleScope<'obj, 'esc, C>>;

  fn deref(&self) -> &Self::Target {
    self.0.scope
  }
}

impl<'borrow, 'scope: 'borrow, 'obj: 'borrow, 'esc: 'obj, C> DerefMut
  for PinnedRef<
    '_,
    TryCatch<'borrow, 'scope, EscapableHandleScope<'obj, 'esc, C>>,
  >
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    /* SAFETY: we're just projecting the pinned reference, it still can't be moved */
    unsafe { self.0.as_mut().get_unchecked_mut().scope }
  }
}

impl<'obj, P> Deref
  for PinnedRef<'_, DisallowJavascriptExecutionScope<'_, 'obj, P>>
{
  type Target = PinnedRef<'obj, P>;
  fn deref(&self) -> &Self::Target {
    self.0.scope
  }
}

impl<P> DerefMut
  for PinnedRef<'_, DisallowJavascriptExecutionScope<'_, '_, P>>
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    /* SAFETY: we're just projecting the pinned reference, it still can't be moved */
    unsafe { self.0.as_mut().get_unchecked_mut().scope }
  }
}

impl<'obj, P> Deref
  for PinnedRef<'_, AllowJavascriptExecutionScope<'_, 'obj, P>>
{
  type Target = PinnedRef<'obj, P>;
  fn deref(&self) -> &Self::Target {
    self.0.scope
  }
}
impl<P> DerefMut for PinnedRef<'_, AllowJavascriptExecutionScope<'_, '_, P>> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    /* SAFETY: we're just projecting the pinned reference, it still can't be moved */
    unsafe { self.0.as_mut().get_unchecked_mut().scope }
  }
}

impl<P> GetIsolate for PinnedRef<'_, P>
where
  P: GetIsolate,
{
  fn get_isolate_ptr(&self) -> *mut RealIsolate {
    self.0.get_isolate_ptr()
  }
}
