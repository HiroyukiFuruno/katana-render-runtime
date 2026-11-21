use super::{
  ManuallyDrop,
  PhantomPinned,
  Pin,
  PinnedRef,
  ScopeInit,
  ScopeStorage,
};

impl<T: ScopeInit> ScopeStorage<T> {
  pub(crate) fn projected(self: Pin<&mut Self>) -> Pin<&mut T> {
    /* SAFETY: we are just projecting to a field, so the scope remains pinned */
    unsafe {
      let self_mut = self.get_unchecked_mut();
      Pin::new_unchecked(&mut self_mut.scope)
    }
  }

  pub fn new(scope: T) -> Self {
    Self {
      inited: false,
      scope: ManuallyDrop::new(scope),
      _pinned: PhantomPinned,
    }
  }

  pub fn init(mut self: Pin<&mut Self>) -> PinnedRef<'_, T> {
    if self.inited {
      /* free old, going to reuse this storage */
      unsafe {
        let self_mut = self.as_mut().get_unchecked_mut();
        self_mut.drop_inner();
        self_mut.inited = false;
      }
    }

    /* hold onto a pointer so we can set this after initialization. we can't use a normal */
    /* mutable reference because the borrow checker will see overlapping borrows. this is */
    /* safe, however, because we lose our mutable reference to the storage in `init_stack` */
    /* as it gets projected to the inner type */
    let inited_ptr =
      unsafe { &raw mut self.as_mut().get_unchecked_mut().inited };
    let ret = T::init_stack(self);
    unsafe { inited_ptr.write(true) };
    PinnedRef(ret)
  }

  /// SAFEFTY: `self.inited` must be true, and therefore must be pinned
  unsafe fn drop_inner(&mut self) {
    unsafe {
      T::deinit(&mut self.scope);
    }
    self.inited = false;
  }
}

impl<T: ScopeInit> Drop for ScopeStorage<T> {
  fn drop(&mut self) {
    if self.inited {
      unsafe {
        self.drop_inner();
      }
    }
  }
}
