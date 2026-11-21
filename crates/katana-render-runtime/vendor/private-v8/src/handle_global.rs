use std::ptr::NonNull;
use std::sync::Arc;

use crate::private_v8::Isolate;
use crate::private_v8::isolate::IsolateLiveness;

use super::{
  Handle, HandleHost, HandleInfo, v8__Global__New, v8__Global__Reset,
};

/// An object reference that is independent of any handle scope. Where
/// a Local handle only lives as long as the HandleScope in which it was
/// allocated, a global handle remains valid until it is dropped.
///
/// A global handle contains a reference to a storage cell within
/// the V8 engine which holds an object value and which is updated by
/// the garbage collector whenever the object is moved.
///
/// You can create a `v8::Local` out of `v8::Global` using
/// `v8::Local::new(scope, global_handle)`.
///
/// Dropping a `Global` off its isolate's home thread defers resetting its
/// V8 storage cell until the home thread next creates or drops a handle,
/// or until the isolate is disposed.
///
/// # Thread safety
///
/// `Global<T>` is [`Send`] and [`Sync`], so the handle may be moved or shared
/// between threads. This does not make the V8 heap object itself concurrently
/// accessible. Cloning, hashing, creating a [`Local`], and comparisons that
/// may involve the same isolate touch V8 and require its home thread.
/// These operations panic when that
/// requirement is not met or when the host isolate has been disposed. Two
/// handles known to belong to different live isolates compare unequal without
/// accessing either isolate. Dropping a `Global` is allowed on any thread and
/// may defer resetting its V8 storage cell as described above.
///
/// `Global<T>` deliberately does not implement [`std::borrow::Borrow<T>`].
/// Such an impl could return a reference that outlives the isolate.
/// Use [`Local::new`] under a
/// handle scope instead. As a consequence, a `HashMap<Global<T>, _>` cannot be
/// queried by `&T`; callers needing allocation-free borrowed lookup should use
/// an embedder-owned stable key rather than the V8 object reference.
///
#[derive(Debug)]
pub struct Global<T> {
  pub(super) data: NonNull<T>,
  /// Liveness cell of the host isolate, shared with the isolate's annex
  /// and every other `Global` it hosts. This reference keeps the cell
  /// readable after the isolate is disposed, so a late drop can observe
  /// the null isolate pointer; the last reference frees the cell.
  pub(super) isolate_liveness: Arc<IsolateLiveness>,
}

impl<T> Global<T> {
  #[inline(always)]
  pub(super) fn assert_access_allowed(&self) {
    self.isolate_liveness.assert_access_allowed();
  }

  /// Construct a new Global from an existing Handle.
  #[inline(always)]
  pub fn new(isolate: &Isolate, handle: impl Handle<Data = T>) -> Self {
    let HandleInfo { data, host } = handle.get_handle_info();
    host.assert_match_isolate(isolate);
    unsafe {
      Self::new_raw(
        isolate as *const Isolate as *mut Isolate,
        data,
        isolate.clone_global_liveness(),
      )
    }
  }

  /// Implementation helper function that contains the code that can be shared
  /// between `Global::new()` and `Global::clone()`. The caller supplies the
  /// liveness reference so `clone` can duplicate the one it already holds
  /// instead of re-fetching the annex.
  #[inline(always)]
  pub(super) unsafe fn new_raw(
    isolate: *mut Isolate,
    data: NonNull<T>,
    isolate_liveness: Arc<IsolateLiveness>,
  ) -> Self {
    let data = data.cast().as_ptr();
    unsafe {
      /* Cheap checkpoint (a relaxed load when empty) so cells dropped by */
      /* threads that couldn't touch the isolate don't pin their JS */
      /* objects indefinitely on isolates that are never locked. */
      isolate_liveness.maybe_drain_deferred_global_resets();
      let data = v8__Global__New((*isolate).as_real_ptr(), data) as *const T;
      let data = NonNull::new_unchecked(data as *mut _);
      Self {
        data,
        isolate_liveness,
      }
    }
  }

  #[inline(always)]
  pub(super) fn get_handle_host(&self) -> HandleHost {
    let isolate = self.isolate_liveness.get_isolate_ptr();
    NonNull::new(isolate)
      .map_or(HandleHost::DisposedIsolate, HandleHost::Isolate)
  }
}

#[path = "handle_global_lifecycle.rs"]
mod lifecycle;
