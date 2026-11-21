use std::fmt::{self, Debug, Formatter};
use std::sync::atomic::AtomicPtr;
use std::sync::Mutex;
use std::ptr::null_mut;

use super::isolate_global_reset::DeferredGlobalReset;
use super::isolate_thread_id::current_thread_id;
use super::{Isolate, RealIsolate};

pub(crate) struct IsolateLiveness {
  isolate: AtomicPtr<RealIsolate>,
  /// The thread that owns the isolate.
  home_thread: usize,
  /// V8 cells whose Rust `Global` owners were dropped by threads that could
  /// not touch the isolate. Drained on `Global` creation, home-thread
  /// `Global` drops, or teardown. `None` after teardown's final drain.
  pub(super) deferred_global_resets: Mutex<Option<Vec<DeferredGlobalReset>>>,
  /// Length hint for `deferred_global_resets`, so drain checkpoints are a
  /// relaxed load in the common (empty) case instead of a mutex
  /// acquisition. Updated under the mutex.
  pub(super) deferred_len: std::sync::atomic::AtomicUsize,
}

impl IsolateLiveness {
  #[inline(always)]
  pub(super) fn new(isolate: &Isolate) -> Self {
    Self {
      isolate: AtomicPtr::new(isolate.as_real_ptr()),
      home_thread: current_thread_id(),
      deferred_global_resets: Mutex::new(Some(Vec::new())),
      deferred_len: std::sync::atomic::AtomicUsize::new(0),
    }
  }

  #[inline(always)]
  pub(super) fn dispose(&self) {
    self
      .isolate
      .store(null_mut(), std::sync::atomic::Ordering::Relaxed);
  }

  #[inline(always)]
  pub(crate) fn get_isolate_ptr(&self) -> *mut RealIsolate {
    self.isolate.load(std::sync::atomic::Ordering::Relaxed)
  }

  /// Only the home thread can access the cell before disposal.
  pub(crate) fn assert_access_allowed(&self) {
    /* A non-shared OwnedIsolate cannot leave its home thread through safe */
    /* Rust. Being on that thread therefore also excludes racing teardown, */
    /* so the hot path needs no mutex. */
    assert!(
      !self.get_isolate_ptr().is_null(),
      "attempt to access Handle hosted by disposed Isolate"
    );
    assert!(
      self.on_home_thread(),
      "accessing a Global requires being on its isolate's thread"
    );
  }

  #[inline(always)]
  pub(crate) fn on_home_thread(&self) -> bool {
    current_thread_id() == self.home_thread
  }

}

impl Debug for IsolateLiveness {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("IsolateLiveness")
      .field("isolate", &self.get_isolate_ptr())
      .finish_non_exhaustive()
  }
}
