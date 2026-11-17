use std::ptr::NonNull;
use crate::private_v8::support::Opaque;

/// Isolate represents an isolated instance of the V8 engine.  V8 isolates have
/// completely separate states.  Objects from one isolate must not be used in
/// other isolates.  The embedder can create multiple isolates and use them in
/// parallel in multiple threads.  An isolate can be entered by at most one
/// thread at any given time.  The Locker/Unlocker API must be used to
/// synchronize.
///
/// rusty_v8 note: Unlike in the C++ API, the Isolate is entered when it is
/// constructed and exited when dropped. Because of that v8::OwnedIsolate
/// instances must be dropped in the reverse order of creation
#[repr(transparent)]
#[derive(Debug)]
pub struct Isolate(NonNull<RealIsolate>);

#[repr(C)]
pub struct RealIsolate(Opaque);

impl Isolate {
  pub(crate) fn as_real_ptr(&self) -> *mut RealIsolate {
    self.0.as_ptr()
  }





  pub unsafe fn from_raw_ptr(ptr: *mut RealIsolate) -> Self {
    Self(NonNull::new(ptr).unwrap())
  }





  #[inline]
  pub(crate) unsafe fn from_non_null(ptr: NonNull<RealIsolate>) -> Self {
    Self(ptr)
  }

  #[inline]
  pub(crate) unsafe fn from_raw_ref(ptr: &NonNull<RealIsolate>) -> &Self {
    /* SAFETY: Isolate is a repr(transparent) wrapper around NonNull<RealIsolate> */
    unsafe { &*(ptr as *const NonNull<RealIsolate> as *const Isolate) }
  }

  #[inline]
  pub(crate) unsafe fn from_raw_ref_mut(
    ptr: &mut NonNull<RealIsolate>,
  ) -> &mut Self {
    /* SAFETY: Isolate is a repr(transparent) wrapper around NonNull<RealIsolate> */
    unsafe { &mut *(ptr as *mut NonNull<RealIsolate> as *mut Isolate) }
  }

}
