use super::*;
use super::inspector_ffi::*;

/* TODO: in C++, this class is intended to be user-extensible, just like */
/* like `Task`, `Client`, `Channel`. In Rust this would ideally also be the */
/* case, but currently to obtain a `UniquePtr<StringBuffer>` is by making a */
/* copy using `StringBuffer::create()`. */
#[repr(C)]
#[derive(Debug)]
pub struct StringBuffer {
  _cxx_vtable: CxxVTable,
}

/* TODO: make it possible to obtain a `UniquePtr<StringBuffer>` directly from */
/* an owned `Vec<u8>` or `Vec<u16>`, */
#[cfg(test)]
impl StringBuffer {
  /* The C++ class definition does not declare `string()` to be a const method, */
  /* therefore we declare self as mutable here. */
  /* TODO: figure out whether it'd be safe to assume a const receiver here. */
  /* That would make it possible to implement `Deref<Target = StringBuffer>`. */
  pub fn string(&self) -> StringView<'_> {
    unsafe { v8_inspector__StringBuffer__string(self) }
  }

  /// This method copies contents.
  pub fn create(source: StringView) -> UniquePtr<StringBuffer> {
    unsafe { v8_inspector__StringBuffer__create(source) }
  }
}

impl Drop for StringBuffer {
  fn drop(&mut self) {
    unsafe { v8_inspector__StringBuffer__DELETE(self) }
  }
}

unsafe impl Send for StringBuffer {}
