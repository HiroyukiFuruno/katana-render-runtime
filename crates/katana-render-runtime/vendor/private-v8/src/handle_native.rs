use crate::private_v8::{Data, isolate::RealIsolate};

unsafe extern "C" {
  pub(super) fn v8__Local__New(
    isolate: *mut RealIsolate,
    other: *const Data,
  ) -> *const Data;
  pub(super) fn v8__Global__New(
    isolate: *mut RealIsolate,
    data: *const Data,
  ) -> *const Data;
  pub(super) fn v8__Global__Reset(data: *const Data);
}
