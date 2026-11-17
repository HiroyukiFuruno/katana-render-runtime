use super::{Isolate, IsolateAnnex};
use std::ptr::NonNull;

impl Isolate {
  /// Prepare annex teardown while keeping `ANNEX_SLOT` pointing at the annex.
  ///
  /// Nulls the `IsolateHandle`'s inner pointer and drops the slot storage. The annex
  /// allocation itself stays alive and `ANNEX_SLOT` keeps pointing at it,
  /// so code that runs during the subsequent V8 teardown can still resolve
  /// the annex through `get_annex()` / `get_annex_mut()`.
  ///
  /// The returned pointer must be passed exactly once to
  /// [`Self::finish_annex_dispose`].
  ///
  /// # Safety
  ///
  /// Called once per isolate, from teardown paths only.
  pub(crate) unsafe fn prepare_annex_for_dispose(&mut self) -> *mut IsolateAnnex {
    let annex_ptr =
      self.get_data_internal(Self::ANNEX_SLOT) as *mut IsolateAnnex;
    assert!(!annex_ptr.is_null());

    /* Each step below operates through the raw pointer rather than a */
    /* long-lived `&mut IsolateAnnex`. The borrows we form here are */
    /* narrowly-scoped expressions that end before any user-controlled */
    /* Drop runs. This matters because slot drops may re-enter the isolate (e.g. via */
    /* `Isolate::thread_safe_handle()`) and resolve the annex through */
    /* `get_annex()`. An outer `&mut IsolateAnnex` held across that */
    /* re-entry would alias the shared borrow they obtain. */

    unsafe { IsolateAnnex::close_handles_for_dispose(annex_ptr) };
    unsafe { IsolateAnnex::drop_slots_for_dispose(annex_ptr) };
    annex_ptr
  }

  /// Free the annex allocation after V8's final teardown.
  ///
  /// `ANNEX_SLOT` is not cleared
  /// because the isolate is gone — its embedder data storage no longer
  /// exists. Dropping the annex releases its reference to the liveness
  /// cell, which is freed with its last reference — this drop, or a
  /// late-dropped `Global`'s.
  ///
  /// # Safety
  ///
  /// `annex_ptr` must be the pointer returned from a matching
  /// [`Self::prepare_annex_for_dispose`] call, and the V8 isolate must
  /// already be fully disposed (so no further callbacks can fire).
  pub(crate) unsafe fn finish_annex_dispose(annex_ptr: *mut IsolateAnnex) {
    unsafe { drop(Box::from_raw(annex_ptr)) };
  }

  #[inline(always)]
  pub(super) fn get_annex(&self) -> &IsolateAnnex {
    let annex_ptr =
      self.get_data_internal(Self::ANNEX_SLOT) as *const IsolateAnnex;
    assert!(!annex_ptr.is_null());
    unsafe { &*annex_ptr }
  }

  #[inline(always)]
  pub(super) fn get_annex_mut(&mut self) -> &mut IsolateAnnex {
    let annex_ptr =
      self.get_data_internal(Self::ANNEX_SLOT) as *mut IsolateAnnex;
    assert!(!annex_ptr.is_null());
    unsafe { &mut *annex_ptr }
  }

  /// Returns a non-null pointer to the isolate's annex data.
  /// This is used by scopes to cache the annex pointer and avoid
  /// repeated FFI calls to `v8__Isolate__GetData`.
  #[inline(always)]
  pub(crate) fn get_annex_ptr(&self) -> NonNull<IsolateAnnex> {
    let annex_ptr =
      self.get_data_internal(Self::ANNEX_SLOT) as *mut IsolateAnnex;
    debug_assert!(!annex_ptr.is_null());
    unsafe { NonNull::new_unchecked(annex_ptr) }
  }

}
