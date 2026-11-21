use super::{
  CreateParams, Isolate, IsolateAnnex, IsolateHandle,
  IsolateLiveness, OwnedIsolate, RealIsolate,
  v8__Isolate__GetNumberOfDataSlots, v8__Isolate__New,
};
use std::sync::Arc;

impl Isolate {
  /* Isolate data slots used internally by rusty_v8. */
  pub(super) const ANNEX_SLOT: u32 = 0;
  pub(super) const INTERNAL_DATA_SLOT_COUNT: u32 = 1;

  #[inline(always)]
  fn assert_embedder_data_slot_count_and_offset_correct(&self) {
    assert!(
      unsafe { v8__Isolate__GetNumberOfDataSlots(self.as_real_ptr()) }
        >= Self::INTERNAL_DATA_SLOT_COUNT
    )
  }

  fn new_impl(params: CreateParams) -> *mut RealIsolate {
    crate::private_v8::V8::assert_initialized();
    let raw_create_params = params.finalize();
    let cxx_isolate = unsafe { v8__Isolate__New(&raw_create_params) };
    let mut isolate = unsafe { Isolate::from_raw_ptr(cxx_isolate) };
    isolate.initialize();
    cxx_isolate
  }

  fn initialize(&mut self) {
    self.assert_embedder_data_slot_count_and_offset_correct();
    self.create_annex();
  }

  /// Creates a new isolate.  Does not change the currently entered
  /// isolate.
  ///
  /// When an isolate is no longer used its resources should be freed
  /// by calling V8::dispose().  Using the delete operator is not allowed.
  ///
  /// V8::initialize() must have run prior to this.
  #[allow(clippy::new_ret_no_self)]
  pub fn new(params: CreateParams) -> OwnedIsolate {
    OwnedIsolate::new(Self::new_impl(params))
  }

  #[inline(always)]
  pub fn thread_safe_handle(&self) -> IsolateHandle {
    self.get_annex().isolate_handle.clone()
  }

  /// One liveness reference for a new `Global`; released when that
  /// `Global` is dropped or converted with `Global::into_raw`. Code that
  /// only needs to observe the cell goes through [`Self::global_liveness`]
  /// instead and stays refcount-free.
  #[inline(always)]
  pub(crate) fn clone_global_liveness(&self) -> Arc<IsolateLiveness> {
    Arc::clone(&self.get_annex().global_liveness)
  }




  fn create_annex(&mut self) {
    let annex_box = Box::new(IsolateAnnex::new(self));
    let annex_ptr = Box::into_raw(annex_box);
    assert!(self.get_data_internal(Self::ANNEX_SLOT).is_null());
    self.set_data_internal(Self::ANNEX_SLOT, annex_ptr as *mut _);
  }

}
