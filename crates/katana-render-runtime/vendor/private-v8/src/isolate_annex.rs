use std::any::TypeId;
use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

use super::{BuildTypeIdHasher, Isolate, IsolateHandle, IsolateLiveness, RawSlot};

pub(crate) struct IsolateAnnex {
  slots: HashMap<TypeId, RawSlot, BuildTypeIdHasher>,
  pub(super) isolate_handle: IsolateHandle,
  /// Liveness cell shared with every `Global` hosted by this isolate.
  /// Globals may be dropped after their host isolate is disposed; each
  /// holds a reference so those late drops can observe the null isolate
  /// pointer, and the last reference — annex or `Global` — frees the
  /// cell instead of leaking it. Refcount traffic is confined to
  /// `Global` creation and destruction, which already pay a V8 FFI
  /// call; liveness reads on access stay refcount-free.
  pub(super) global_liveness: Arc<IsolateLiveness>,
}

impl IsolateAnnex {
  pub(super) fn new(isolate: &Isolate) -> Self {
    Self {
      slots: HashMap::default(),
      isolate_handle: IsolateHandle::new(isolate),
      global_liveness: Arc::new(IsolateLiveness::new(isolate)),
    }
  }

  #[inline(always)]
  pub(super) fn global_liveness(&self) -> &IsolateLiveness {
    &self.global_liveness
  }

  /// Close Global access and dispose the shared handle before V8 teardown.
  ///
  /// # Safety
  ///
  /// `annex_ptr` must be non-null and point to the live annex in ANNEX_SLOT.
  pub(super) unsafe fn close_handles_for_dispose(annex_ptr: *mut Self) {
    unsafe {
      (*annex_ptr)
        .global_liveness()
        .close_deferred_global_resets();
      (*annex_ptr).global_liveness().dispose();
      (*annex_ptr).isolate_handle.dispose();
    }
  }

  /// Take the slots before dropping user-owned slot values.
  ///
  /// # Safety
  ///
  /// `annex_ptr` must be non-null and point to the live annex in ANNEX_SLOT.
  pub(super) unsafe fn drop_slots_for_dispose(annex_ptr: *mut Self) {
    /* Take slots through a short raw-pointer borrow. Dropping slots may */
    /* re-enter the annex, so no `&mut IsolateAnnex` may span that Drop. */
    let slots = unsafe { std::mem::take(&mut (*annex_ptr).slots) };
    drop(slots);
  }

  #[inline(always)]
  pub(crate) fn get_slot<T: 'static>(&self) -> Option<&T> {
    self
      .slots
      .get(&TypeId::of::<T>())
      .map(|slot| unsafe { slot.borrow::<T>() })
  }

  #[inline(always)]
  pub(crate) fn set_slot<T: 'static>(&mut self, value: T) -> bool {
    self
      .slots
      .insert(TypeId::of::<T>(), RawSlot::new(value))
      .is_none()
  }

}

impl Debug for IsolateAnnex {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("IsolateAnnex")
      .field("isolate_handle", &self.isolate_handle)
      .finish()
  }
}
