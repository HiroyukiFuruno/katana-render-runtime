use super::isolate_global_reset::DeferredGlobalReset;
use super::isolate_liveness::IsolateLiveness;
use crate::private_v8::Data;
use std::ptr::NonNull;

impl IsolateLiveness {
  /// Release a `Global`'s cell now if the current thread may touch the
  /// isolate; otherwise queue it for the home thread or isolate teardown.
  ///
  /// While the queue is `Some`, teardown has not started: its final drain
  /// closes the queue under this mutex before the isolate is disposed, so
  /// the isolate pointer read below stays valid for the duration of this
  /// call. The null check is belt and braces on top of that invariant: a
  /// disposed isolate means the cell is already gone.
  pub(crate) fn reset_or_defer_global(&self, data: NonNull<Data>) {
    let mut q = self.deferred_global_resets.lock().unwrap();
    let Some(v) = q.as_mut() else { return };
    if self.get_isolate_ptr().is_null() {
      return;
    }
    if self.on_home_thread() {
      drop(q);
      DeferredGlobalReset(data).reset();
    } else {
      v.push(DeferredGlobalReset(data));
      self
        .deferred_len
        .store(v.len(), std::sync::atomic::Ordering::Relaxed);
    }
  }

  /// Cheap drain checkpoint: a relaxed load when the queue is empty.
  /// The caller must be on the isolate's home thread.
  #[inline(always)]
  pub(crate) fn maybe_drain_deferred_global_resets(&self) {
    if self.deferred_len.load(std::sync::atomic::Ordering::Relaxed) > 0 {
      self.drain_deferred_global_resets();
    }
  }

  /// Called on the isolate's home thread.
  pub(crate) fn drain_deferred_global_resets(&self) {
    let drained = {
      let mut q = self.deferred_global_resets.lock().unwrap();
      self
        .deferred_len
        .store(0, std::sync::atomic::Ordering::Relaxed);
      match q.as_mut() {
        Some(v) => std::mem::take(v),
        None => return,
      }
    };
    for reset in drained {
      reset.reset();
    }
  }

  /// Final drain during teardown on the isolate's home thread. Late
  /// droppers that arrive after this see a closed queue and do nothing;
  /// their cells are freed with the isolate.
  pub(crate) fn close_deferred_global_resets(&self) {
    let drained = {
      let mut q = self.deferred_global_resets.lock().unwrap();
      self
        .deferred_len
        .store(0, std::sync::atomic::Ordering::Relaxed);
      q.take()
    };
    for reset in drained.into_iter().flatten() {
      reset.reset();
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::private_v8::{Global, Isolate, Primitive, initialize_v8, scope};
  use std::sync::atomic::Ordering;

  #[test]
  fn off_thread_global_drop_is_drained_on_the_home_thread() {
    initialize_v8();
    let mut isolate = Isolate::new(Default::default());
    let liveness = isolate.clone_global_liveness();
    scope!(let scope, &mut isolate);
    let value = Primitive::null(scope);
    let global = Global::new(scope, value);

    std::thread::spawn(move || drop(global)).join().unwrap();
    assert_eq!(liveness.deferred_len.load(Ordering::Relaxed), 1);

    let _next_global = Global::new(scope, value);
    assert_eq!(liveness.deferred_len.load(Ordering::Relaxed), 0);
  }
}
