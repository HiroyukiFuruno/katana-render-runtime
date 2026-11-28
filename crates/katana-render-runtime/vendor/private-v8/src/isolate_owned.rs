use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use super::{Isolate, RealIsolate};
use super::isolate_owned_type::OwnedIsolate;
use crate::private_v8::{Platform, V8};

impl OwnedIsolate {
  pub(crate) fn new(cxx_isolate: *mut RealIsolate) -> Self {
    let isolate = Self::new_already_entered(cxx_isolate);
    unsafe {
      isolate.enter();
    }
    isolate
  }

  pub(crate) fn new_already_entered(cxx_isolate: *mut RealIsolate) -> Self {
    let cxx_isolate = NonNull::new(cxx_isolate).unwrap();
    let owned_isolate: OwnedIsolate = Self { cxx_isolate };
    /* owned_isolate.init_scope_root(); */
    owned_isolate
  }
}

impl Drop for OwnedIsolate {
  fn drop(&mut self) {
    self.dispose_inner();
  }
}

impl OwnedIsolate {
  fn dispose_inner(&mut self) {
    self.assert_drop_state();
    unsafe { self.exit() };
    self.finish_disposal();
  }

  fn assert_drop_state(&mut self) {
    unsafe {
      /* Safety: We need to check `this == Isolate::GetCurrent()` before calling exit() */
      assert!(
        std::ptr::eq(self.cxx_isolate.as_mut(), super::v8__Isolate__GetCurrent()),
        "v8::OwnedIsolate instances must be dropped in the reverse order of creation. They are entered upon creation and exited upon being dropped."
      );
    }
  }

  fn finish_disposal(&mut self) {
    unsafe {
      let annex_ptr = self.prepare_annex_for_dispose();
      Platform::notify_isolate_shutdown(&V8::get_current_platform(), self);
      self.dispose();
      Isolate::finish_annex_dispose(annex_ptr);
    }
  }


}

impl Deref for OwnedIsolate {
  type Target = Isolate;
  fn deref(&self) -> &Self::Target {
    unsafe {
      std::mem::transmute::<&NonNull<RealIsolate>, &Isolate>(&self.cxx_isolate)
    }
  }
}

impl DerefMut for OwnedIsolate {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe {
      std::mem::transmute::<&mut NonNull<RealIsolate>, &mut Isolate>(
        &mut self.cxx_isolate,
      )
    }
  }
}
