use crate::private_v8::{OnFailure, isolate::RealIsolate};
use std::{mem::MaybeUninit, ptr::NonNull};

#[repr(C)]
#[derive(Debug)]
pub(in super::super) struct DisallowJavascriptExecutionScope(
  [MaybeUninit<usize>; 2],
);

impl DisallowJavascriptExecutionScope {
  /// Creates an uninitialized `DisallowJavascriptExecutionScope`.
  ///
  /// This function is marked unsafe because the caller must ensure that the
  /// returned value isn't dropped before `init()` has been called.
  #[inline]
  #[cfg(test)]
  pub unsafe fn uninit() -> Self {
    /* SAFETY: All bit patterns are valid, since the struct is made up of MaybeUninit. */
    Self(unsafe { MaybeUninit::uninit().assume_init() })
  }

  /// This function is marked unsafe because `init()` must be called exactly
  /// once, no more and no less, after creating a
  /// `DisallowJavascriptExecutionScope` value with
  /// `DisallowJavascriptExecutionScope::uninit()`.
  #[inline]
  pub unsafe fn init(
    &mut self,
    isolate: NonNull<RealIsolate>,
    on_failure: OnFailure,
  ) {
    let buf = NonNull::from(self).cast();
    unsafe {
      v8__DisallowJavascriptExecutionScope__CONSTRUCT(
        buf.as_ptr(),
        isolate.as_ptr(),
        on_failure,
      );
    }
  }
}

impl Drop for DisallowJavascriptExecutionScope {
  #[inline(always)]
  fn drop(&mut self) {
    unsafe { v8__DisallowJavascriptExecutionScope__DESTRUCT(self) };
  }
}

#[repr(C)]
#[derive(Debug)]
pub(in super::super) struct AllowJavascriptExecutionScope(
  [MaybeUninit<usize>; 2],
);

impl AllowJavascriptExecutionScope {
  /// Creates an uninitialized `AllowJavascriptExecutionScope`.
  ///
  /// This function is marked unsafe because the caller must ensure that the
  /// returned value isn't dropped before `init()` has been called.
  #[inline]
  #[cfg(test)]
  pub unsafe fn uninit() -> Self {
    Self(unsafe { MaybeUninit::uninit().assume_init() })
  }

  /// This function is marked unsafe because `init()` must be called exactly
  /// once, no more and no less, after creating an
  /// `AllowJavascriptExecutionScope` value with
  /// `AllowJavascriptExecutionScope::uninit()`.
  #[inline]
  pub unsafe fn init(&mut self, isolate: NonNull<RealIsolate>) {
    unsafe {
      let buf = NonNull::from(self).cast();
      v8__AllowJavascriptExecutionScope__CONSTRUCT(
        buf.as_ptr(),
        isolate.as_ptr(),
      );
    }
  }
}

impl Drop for AllowJavascriptExecutionScope {
  #[inline(always)]
  fn drop(&mut self) {
    unsafe { v8__AllowJavascriptExecutionScope__DESTRUCT(self) };
  }
}

unsafe extern "C" {
  fn v8__DisallowJavascriptExecutionScope__CONSTRUCT(
    buf: *mut MaybeUninit<DisallowJavascriptExecutionScope>,
    isolate: *mut RealIsolate,
    on_failure: OnFailure,
  );
  pub(in super::super) fn v8__DisallowJavascriptExecutionScope__DESTRUCT(
    this: *mut DisallowJavascriptExecutionScope,
  );

  fn v8__AllowJavascriptExecutionScope__CONSTRUCT(
    buf: *mut MaybeUninit<AllowJavascriptExecutionScope>,
    isolate: *mut RealIsolate,
  );
  pub(in super::super) fn v8__AllowJavascriptExecutionScope__DESTRUCT(
    this: *mut AllowJavascriptExecutionScope,
  );
}
