use super::*;
use crate::private_v8::support::Opaque;

#[repr(C)]
pub(super) struct RawSerializable(Opaque);

pub struct Serializable {
  pub(super) ptr: *mut RawSerializable,
}

impl Serializable {
}

impl Drop for Serializable {
  fn drop(&mut self) {
    unsafe {
      crdtp__Serializable__DELETE(self.ptr);
    }
  }
}
