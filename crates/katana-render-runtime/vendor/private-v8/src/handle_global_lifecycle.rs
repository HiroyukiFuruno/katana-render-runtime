use super::{Global, Handle, HandleInfo, Isolate, IsolateLiveness, v8__Global__Reset};
use std::sync::Arc;

/* A `Global` only touches V8 through methods that either take a scope or */
/* `&Isolate` argument on the isolate's thread, or are guarded through */
/* `IsolateLiveness`: `clone`, */
/* `eq` and `hash` assert the current thread may touch the isolate, and */
/* `drop` releases the cell immediately when it may, deferring to the */
/* liveness queue otherwise. The `Arc<IsolateLiveness>` is `Send + Sync` */
/* on its own; these impls are needed for the raw `data` pointer. */
unsafe impl<T> Send for Global<T> {}
unsafe impl<T> Sync for Global<T> {}

/* The blanket impls above would mask it if `IsolateLiveness` ever lost */
/* these auto traits, so assert them where the claim is made. */
const _: () = {
  const fn assert_send_sync<T: Send + Sync>() {}
  assert_send_sync::<IsolateLiveness>()
};

impl<T> Clone for Global<T> {
  fn clone(&self) -> Self {
    self.assert_access_allowed();
    let HandleInfo { data, host } = self.get_handle_info();
    let mut isolate = unsafe { Isolate::from_non_null(host.get_isolate()) };
    let isolate_liveness = Arc::clone(&self.isolate_liveness);
    unsafe { Self::new_raw(isolate.as_mut(), data, isolate_liveness) }
  }
}

impl<T> Drop for Global<T> {
  fn drop(&mut self) {
    /* The `Arc` field keeps the cell alive for this whole body; its own */
    /* decrement (and a possible final free) runs after it. */
    let liveness = &*self.isolate_liveness;
    if liveness.get_isolate_ptr().is_null() {
      /* This `Global` handle is associated with an `Isolate` that has already */
      /* been disposed. */
    } else if liveness.on_home_thread() {
      /* Destroy the storage cell that contains the contents of this Global. */
      unsafe { v8__Global__Reset(self.data.cast().as_ptr()) };
      liveness.maybe_drain_deferred_global_resets();
    } else {
      /* Defer the cell reset to the home thread or isolate teardown. */
      liveness.reset_or_defer_global(self.data.cast());
    }
  }
}
