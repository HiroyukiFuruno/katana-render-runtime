#[cfg(test)]
use crate::private_v8::{Data, Primitive};
#[cfg(test)]
use std::ptr;
/// The `raw` module contains prototypes for all the `extern C` functions that
/// are used in the scope module, as well as definitions for the types they operate on.
use crate::private_v8::{
  Context, Local, Value, isolate::RealIsolate,
};
use std::num::NonZeroUsize;
use std::{
  mem::MaybeUninit,
  ptr::NonNull,
};

#[path = "raw_exception.rs"]
mod exception;
pub(super) use exception::{
  TryCatch, v8__TryCatch__DESTRUCT,
  v8__TryCatch__Exception, v8__TryCatch__HasTerminated,
  v8__TryCatch__Message, v8__TryCatch__StackTrace,
};
#[cfg(test)]
pub(super) use exception::{v8__TryCatch__HasCaught, v8__TryCatch__Reset};

#[path = "raw_execution.rs"]
mod execution;
pub(super) use execution::{
  AllowJavascriptExecutionScope, DisallowJavascriptExecutionScope,
  v8__AllowJavascriptExecutionScope__DESTRUCT,
  v8__DisallowJavascriptExecutionScope__DESTRUCT,
};

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub(super) struct Address(NonZeroUsize);

#[derive(Debug)]
#[repr(transparent)]
pub(super) struct ContextScope {
  pub(super) entered_context: NonNull<Context>,
}

impl ContextScope {
  pub fn new(context: Local<Context>) -> Self {
    unsafe { v8__Context__Enter(&*context) };
    Self {
      entered_context: context.as_non_null(),
    }
  }
}

impl Drop for ContextScope {
  #[inline(always)]
  fn drop(&mut self) {
    unsafe { v8__Context__Exit(self.entered_context.as_ptr()) };
  }
}

#[cfg(any())]
pub const HANDLE_SCOPE_SIZE: usize = 4;
#[cfg(not(any()))]
pub const HANDLE_SCOPE_SIZE: usize = 3;

#[repr(C)]
#[derive(Debug)]
pub(super) struct HandleScope([MaybeUninit<usize>; HANDLE_SCOPE_SIZE]);

impl HandleScope {
  /// Creates an uninitialized `HandleScope`.
  ///
  /// This function is marked unsafe because the caller must ensure that the
  /// returned value isn't dropped before `init()` has been called.
  #[inline(always)]
  pub unsafe fn uninit() -> Self {
    unsafe { MaybeUninit::uninit().assume_init() }
  }

  /// This function is marked unsafe because `init()` must be called exactly
  /// once, no more and no less, after creating a `HandleScope` value with
  /// `HandleScope::uninit()`.
  #[inline(always)]
  pub unsafe fn init(&mut self, isolate: NonNull<RealIsolate>) {
    let buf = NonNull::from(self).as_ptr().cast();
    unsafe {
      v8__HandleScope__CONSTRUCT(buf, isolate.as_ptr());
    }
  }
}

impl Drop for HandleScope {
  #[inline(always)]
  fn drop(&mut self) {
    unsafe { v8__HandleScope__DESTRUCT(self) };
  }
}

#[repr(transparent)]
#[derive(Debug)]
pub(super) struct EscapeSlot(NonNull<Address>);

#[cfg(test)]
impl EscapeSlot {
  pub fn new(isolate: NonNull<RealIsolate>) -> Self {
    unsafe {
      let undefined = v8__Undefined(isolate.as_ptr()) as *const _;
      let local = v8__Local__New(isolate.as_ptr(), undefined);
      let slot_address_ptr = local as *const Address as *mut _;
      let slot_address_nn = NonNull::new_unchecked(slot_address_ptr);
      Self(slot_address_nn)
    }
  }

  pub fn escape<'e, T>(self, value: Local<'_, T>) -> Local<'e, T>
  where
    for<'l> Local<'l, T>: Into<Local<'l, Data>>,
  {
    const {
      assert!(size_of::<Self>() == size_of::<Local<T>>());
      assert!(align_of::<Self>() == align_of::<Local<T>>());
    }
    unsafe {
      let undefined = Local::<Value>::from_non_null(self.0.cast());
      debug_assert!(undefined.is_undefined());
      let value_address = *(&*value as *const T as *const Address);
      ptr::write(self.0.as_ptr(), value_address);
      Local::from_non_null(self.0.cast())
    }
  }
}

unsafe extern "C" {
  pub(super) fn v8__Isolate__GetCurrent() -> *mut RealIsolate;
  pub(super) fn v8__Isolate__GetCurrentContext(
    isolate: *mut RealIsolate,
  ) -> *const Context;
  pub(super) fn v8__Isolate__ThrowException(
    isolate: *mut RealIsolate,
    exception: *const Value,
  ) -> *const Value;

  pub(super) fn v8__Context__Enter(this: *const Context);
  pub(super) fn v8__Context__Exit(this: *const Context);

  pub(super) fn v8__HandleScope__CONSTRUCT(
    buf: *mut MaybeUninit<HandleScope>,
    isolate: *mut RealIsolate,
  );
  pub(super) fn v8__HandleScope__DESTRUCT(this: *mut HandleScope);

  #[cfg(test)]
  pub(super) fn v8__Local__New(
    isolate: *mut RealIsolate,
    other: *const Data,
  ) -> *const Data;
  #[cfg(test)]
  pub(super) fn v8__Undefined(isolate: *mut RealIsolate) -> *const Primitive;
}
