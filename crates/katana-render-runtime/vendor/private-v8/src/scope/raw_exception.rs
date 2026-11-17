use crate::private_v8::{Context, Message, Value, isolate::RealIsolate};
use std::{mem::MaybeUninit, ptr::NonNull};

#[repr(C)]
#[derive(Debug)]
pub(in super::super) struct TryCatch([MaybeUninit<usize>; TRY_CATCH_ABI_WORDS]);

const TRY_CATCH_ABI_WORDS: usize = 6;

impl TryCatch {
  /// Creates an uninitialized `TryCatch`.
  ///
  /// This function is marked unsafe because the caller must ensure that the
  /// returned value isn't dropped before `init()` has been called.
  pub unsafe fn uninit() -> Self {
    /* SAFETY: All bit patterns are valid, since the struct is made up of MaybeUninit. */
    Self(unsafe { MaybeUninit::uninit().assume_init() })
  }

  /// This function is marked unsafe because `init()` must be called exactly
  /// once, no more and no less, after creating a `TryCatch` value with
  /// `TryCatch::uninit()`.
  pub unsafe fn init(&mut self, isolate: NonNull<RealIsolate>) {
    let buf = NonNull::from(self).cast();
    unsafe {
      v8__TryCatch__CONSTRUCT(buf.as_ptr(), isolate.as_ptr());
    }
  }
}

impl Drop for TryCatch {
  #[inline(always)]
  fn drop(&mut self) {
    unsafe { v8__TryCatch__DESTRUCT(self) };
  }
}

unsafe extern "C" {
  fn v8__TryCatch__CONSTRUCT(
    buf: *mut MaybeUninit<TryCatch>,
    isolate: *mut RealIsolate,
  );
  pub(in super::super) fn v8__TryCatch__DESTRUCT(this: *mut TryCatch);
  #[cfg(test)]
  pub(in super::super) fn v8__TryCatch__HasCaught(
    this: *const TryCatch,
  ) -> bool;
  pub(in super::super) fn v8__TryCatch__HasTerminated(
    this: *const TryCatch,
  ) -> bool;
  #[cfg(test)]
  pub(in super::super) fn v8__TryCatch__Reset(this: *mut TryCatch);
  pub(in super::super) fn v8__TryCatch__Exception(
    this: *const TryCatch,
  ) -> *const Value;
  pub(in super::super) fn v8__TryCatch__StackTrace(
    this: *const TryCatch,
    context: *const Context,
  ) -> *const Value;
  pub(in super::super) fn v8__TryCatch__Message(
    this: *const TryCatch,
  ) -> *const Message;
}
