#[cfg(test)]
use std::ffi::c_void;

use crate::private_v8::support::{
  Opaque, Shared, SharedPtrBase, UniquePtr, UniqueRef,
};

#[cfg(test)]
use crate::private_v8::support::long;

unsafe extern "C" {
  fn v8__ArrayBuffer__Allocator__NewDefaultAllocator() -> *mut Allocator;
  fn v8__ArrayBuffer__Allocator__DELETE(this: *mut Allocator);
  fn std__shared_ptr__v8__ArrayBuffer__Allocator__COPY(
    ptr: *const SharedPtrBase<Allocator>,
  ) -> SharedPtrBase<Allocator>;
  fn std__shared_ptr__v8__ArrayBuffer__Allocator__CONVERT__std__unique_ptr(
    unique_ptr: UniquePtr<Allocator>,
  ) -> SharedPtrBase<Allocator>;
  fn std__shared_ptr__v8__ArrayBuffer__Allocator__get(
    ptr: *const SharedPtrBase<Allocator>,
  ) -> *mut Allocator;
  fn std__shared_ptr__v8__ArrayBuffer__Allocator__reset(
    ptr: *mut SharedPtrBase<Allocator>,
  );
  #[cfg(test)]
  fn std__shared_ptr__v8__ArrayBuffer__Allocator__use_count(
    ptr: *const SharedPtrBase<Allocator>,
  ) -> long;
}

/* Rust allocator feature is only available in non-sandboxed mode */
#[cfg(test)]
unsafe extern "C" {
  fn v8__ArrayBuffer__Allocator__NewRustAllocator(
    handle: *const c_void,
    vtable: *const RustAllocatorVtable<c_void>,
  ) -> *mut Allocator;
}

/// A thread-safe allocator that V8 uses to allocate |ArrayBuffer|'s memory.
/// The allocator is a global V8 setting. It has to be set via
/// Isolate::CreateParams.
///
/// Memory allocated through this allocator by V8 is accounted for as external
/// memory by V8. Note that V8 keeps track of the memory for all internalized
/// |ArrayBuffer|s. Responsibility for tracking external memory (using
/// Isolate::AdjustAmountOfExternalAllocatedMemory) is handed over to the
/// embedder upon externalization and taken over upon internalization (creating
/// an internalized buffer from an existing buffer).
///
/// Note that it is unsafe to call back into V8 from any of the allocator
/// functions.
///
/// This is called v8::ArrayBuffer::Allocator in C++. Rather than use the
/// namespace array_buffer, which will contain only the Allocator we opt in Rust
/// to allow it to live in the top level: v8::Allocator
#[repr(C)]
#[derive(Debug)]
pub struct Allocator(Opaque);

/// A wrapper around the V8 Allocator class.
#[cfg(test)]
#[repr(C)]
pub struct RustAllocatorVtable<T> {
  pub allocate: unsafe extern "C" fn(handle: &T, len: usize) -> *mut c_void,
  pub allocate_uninitialized:
    unsafe extern "C" fn(handle: &T, len: usize) -> *mut c_void,
  pub free: unsafe extern "C" fn(handle: &T, data: *mut c_void, len: usize),
  pub drop: unsafe extern "C" fn(handle: *const T),
}

impl Shared for Allocator {
  fn clone(ptr: &SharedPtrBase<Self>) -> SharedPtrBase<Self> {
    unsafe { std__shared_ptr__v8__ArrayBuffer__Allocator__COPY(ptr) }
  }
  fn from_unique_ptr(unique_ptr: UniquePtr<Self>) -> SharedPtrBase<Self> {
    unsafe {
      std__shared_ptr__v8__ArrayBuffer__Allocator__CONVERT__std__unique_ptr(
        unique_ptr,
      )
    }
  }
  fn get(ptr: &SharedPtrBase<Self>) -> *const Self {
    unsafe { std__shared_ptr__v8__ArrayBuffer__Allocator__get(ptr) }
  }
  fn reset(ptr: &mut SharedPtrBase<Self>) {
    unsafe { std__shared_ptr__v8__ArrayBuffer__Allocator__reset(ptr) }
  }
  #[cfg(test)]
  fn use_count(ptr: &SharedPtrBase<Self>) -> long {
    unsafe { std__shared_ptr__v8__ArrayBuffer__Allocator__use_count(ptr) }
  }
}

impl Allocator {
  /// malloc/free based convenience allocator.
  #[inline(always)]
  pub fn new_default() -> UniqueRef<Self> {
    unsafe {
      UniqueRef::from_raw(v8__ArrayBuffer__Allocator__NewDefaultAllocator())
    }
  }

  /// Creates an allocator managed by Rust code.
  ///
  /// Marked `unsafe` because the caller must ensure that `handle` is valid and matches what `vtable` expects.
  ///
  /// Not usable in sandboxed mode
  #[inline(always)]
  #[cfg(test)]
  pub unsafe fn new_rust<T: Sized + Send + Sync + 'static>(
    handle: *const T,
    vtable: &'static RustAllocatorVtable<T>,
  ) -> UniqueRef<Self> {
    unsafe {
      UniqueRef::from_raw(v8__ArrayBuffer__Allocator__NewRustAllocator(
        handle as *const c_void,
        vtable as *const RustAllocatorVtable<T>
          as *const RustAllocatorVtable<c_void>,
      ))
    }
  }
}

#[cfg(test)]
mod rust_allocator_tests {
  use super::{Allocator, RustAllocatorVtable, c_void};
  use std::alloc::{Layout, alloc, alloc_zeroed, dealloc};
  use std::ptr::null_mut;
  use std::sync::Arc;
  use std::sync::atomic::{AtomicUsize, Ordering};

  const RELEASE_MARKER: usize = 42;

  fn allocation_layout(bytes: usize) -> Option<Layout> {
    Layout::from_size_align(bytes.max(1), align_of::<u128>()).ok()
  }

  unsafe extern "C" fn allocate(_: &AtomicUsize, bytes: usize) -> *mut c_void {
    allocation_layout(bytes)
      .map_or(null_mut(), |layout| unsafe { alloc_zeroed(layout).cast() })
  }

  unsafe extern "C" fn allocate_uninitialized(
    _: &AtomicUsize,
    bytes: usize,
  ) -> *mut c_void {
    allocation_layout(bytes)
      .map_or(null_mut(), |layout| unsafe { alloc(layout).cast() })
  }

  unsafe extern "C" fn free(
    _: &AtomicUsize,
    memory: *mut c_void,
    bytes: usize,
  ) {
    if !memory.is_null()
      && let Some(layout) = allocation_layout(bytes)
    {
      unsafe { dealloc(memory.cast(), layout) };
    }
  }
  unsafe extern "C" fn drop(x: *const AtomicUsize) {
    unsafe {
      let arc = Arc::from_raw(x);
      arc.store(RELEASE_MARKER, Ordering::SeqCst);
    }
  }

  #[test]
  fn releases_owned_allocation() {
    let retval = Arc::new(AtomicUsize::new(0));
    let vtable: &'static RustAllocatorVtable<AtomicUsize> =
      &RustAllocatorVtable {
        allocate,
        allocate_uninitialized,
        free,
        drop,
      };
    unsafe { Allocator::new_rust(Arc::into_raw(retval.clone()), vtable) };
    assert_eq!(retval.load(Ordering::SeqCst), RELEASE_MARKER);
    assert_eq!(Arc::strong_count(&retval), 1);
  }
}

#[test]
fn test_default_allocator() {
  crate::private_v8::initialize_v8();
  Allocator::new_default();
}

impl Drop for Allocator {
  fn drop(&mut self) {
    unsafe { v8__ArrayBuffer__Allocator__DELETE(self) };
  }
}
