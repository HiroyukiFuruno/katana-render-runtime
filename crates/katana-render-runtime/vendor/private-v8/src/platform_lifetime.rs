use crate::private_v8::support::Opaque;
use crate::private_v8::support::Shared;
use crate::private_v8::support::SharedPtrBase;
use crate::private_v8::support::UniquePtr;
#[cfg(test)]
use crate::private_v8::support::long;

unsafe extern "C" {
  fn v8__Platform__DELETE(this: *mut Platform);
  fn std__shared_ptr__v8__Platform__CONVERT__std__unique_ptr(
    unique_ptr: UniquePtr<Platform>,
  ) -> SharedPtrBase<Platform>;
  fn std__shared_ptr__v8__Platform__get(
    ptr: *const SharedPtrBase<Platform>,
  ) -> *mut Platform;
  fn std__shared_ptr__v8__Platform__COPY(
    ptr: *const SharedPtrBase<Platform>,
  ) -> SharedPtrBase<Platform>;
  fn std__shared_ptr__v8__Platform__reset(ptr: *mut SharedPtrBase<Platform>);
  #[cfg(test)]
  fn std__shared_ptr__v8__Platform__use_count(
    ptr: *const SharedPtrBase<Platform>,
  ) -> long;
}

#[repr(C)]
#[derive(Debug)]
pub struct Platform(Opaque);

impl Shared for Platform {
  fn from_unique_ptr(unique_ptr: UniquePtr<Self>) -> SharedPtrBase<Self> {
    unsafe {
      std__shared_ptr__v8__Platform__CONVERT__std__unique_ptr(unique_ptr)
    }
  }
  fn get(ptr: &SharedPtrBase<Self>) -> *const Self {
    unsafe { std__shared_ptr__v8__Platform__get(ptr) }
  }
  fn clone(ptr: &SharedPtrBase<Self>) -> SharedPtrBase<Self> {
    unsafe { std__shared_ptr__v8__Platform__COPY(ptr) }
  }
  fn reset(ptr: &mut SharedPtrBase<Self>) {
    unsafe { std__shared_ptr__v8__Platform__reset(ptr) }
  }
  #[cfg(test)]
  fn use_count(ptr: &SharedPtrBase<Self>) -> long {
    unsafe { std__shared_ptr__v8__Platform__use_count(ptr) }
  }
}

impl Drop for Platform {
  fn drop(&mut self) {
    unsafe { v8__Platform__DELETE(self) };
  }
}
