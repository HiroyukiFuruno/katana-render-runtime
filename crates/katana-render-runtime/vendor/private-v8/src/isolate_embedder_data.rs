use super::{Isolate, v8__Isolate__GetData, v8__Isolate__SetData};
use std::ffi::c_void;

impl Isolate {
  #[inline(always)]
  pub(crate) fn get_data_internal(&self, slot: u32) -> *mut c_void {
    unsafe { v8__Isolate__GetData(self.as_real_ptr(), slot) }
  }

  #[inline(always)]
  pub(crate) fn set_data_internal(&mut self, slot: u32, data: *mut c_void) {
    unsafe { v8__Isolate__SetData(self.as_real_ptr(), slot, data) }
  }
}
