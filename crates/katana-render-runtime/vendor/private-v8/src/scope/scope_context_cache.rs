use super::{
  CallbackScope, ClearCachedContext, EscapableHandleScope, HandleScope,
  PinnedRef,
};

impl<C> ClearCachedContext for HandleScope<'_, C> {
  fn clear_cached_context(&self) {
    self.context.set(None);
  }
}

impl<C> ClearCachedContext for CallbackScope<'_, C> {
  fn clear_cached_context(&self) {}
}

impl<P: ClearCachedContext> ClearCachedContext for PinnedRef<'_, P> {
  fn clear_cached_context(&self) {
    self.0.clear_cached_context();
  }
}

impl<C> ClearCachedContext for EscapableHandleScope<'_, '_, C> {
  fn clear_cached_context(&self) {
    self.context.set(None);
  }
}
