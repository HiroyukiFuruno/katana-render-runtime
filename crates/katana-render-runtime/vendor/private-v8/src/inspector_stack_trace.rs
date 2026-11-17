use super::*;
use super::inspector_ffi::*;

#[repr(C)]
#[derive(Debug)]
pub struct V8StackTrace {
  _cxx_vtable: CxxVTable,
}

impl Drop for V8StackTrace {
  fn drop(&mut self) {
    unsafe { v8_inspector__V8StackTrace__DELETE(self) };
  }
}

/* TODO(bnoordhuis) This needs to be fleshed out more but that can wait */
/* until it's actually needed. */
