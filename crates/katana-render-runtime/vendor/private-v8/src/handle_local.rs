use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;

use crate::private_v8::scope::PinScope;

use super::{Handle, HandleInfo, v8__Local__New};

/// An object reference managed by the v8 garbage collector.
///
/// All objects returned from v8 have to be tracked by the garbage
/// collector so that it knows that the objects are still alive.  Also,
/// because the garbage collector may move objects, it is unsafe to
/// point directly to an object.  Instead, all objects are stored in
/// handles which are known by the garbage collector and updated
/// whenever an object moves.  Handles should always be passed by value
/// (except in cases like out-parameters) and they should never be
/// allocated on the heap.
///
/// There are two types of handles: local and persistent handles.
///
/// Local handles are light-weight and transient and typically used in
/// local operations.  They are managed by HandleScopes. That means that a
/// HandleScope must exist on the stack when they are created and that they are
/// only valid inside of the `HandleScope` active during their creation.
/// For passing a local handle to an outer `HandleScope`, an
/// `EscapableHandleScope` and its `Escape()` method must be used.
///
/// Persistent handles can be used when storing objects across several
/// independent operations and have to be explicitly deallocated when they're no
/// longer used.
///
/// It is safe to extract the object stored in the handle by
/// dereferencing the handle (for instance, to extract the `*Object` from
/// a `Local<Object>`); the value will still be governed by a handle
/// behind the scenes and the same rules apply to these values as to
/// their handles.
///
/// Note: Local handles in Rusty V8 differ from the V8 C++ API in that they are
/// never empty. In situations where empty handles are needed, use
/// `Option<Local>`.
#[repr(C)]
#[derive(Debug)]
pub struct Local<'s, T>(NonNull<T>, PhantomData<&'s ()>);

impl<'s, T> Local<'s, T> {
  /// Construct a new Local from an existing Handle.
  #[inline(always)]
  pub fn new<'i>(
    scope: &PinScope<'s, 'i, ()>,
    handle: impl Handle<Data = T>,
  ) -> Local<'s, T> {
    let HandleInfo { data, host } = handle.get_handle_info();
    host.assert_match_isolate(scope);
    unsafe {
      scope.cast_local(|sd| {
        v8__Local__New(sd.get_isolate_ptr(), data.cast().as_ptr()) as *const T
      })
    }
    .unwrap()
  }

  #[inline(always)]
  pub(crate) unsafe fn from_raw(ptr: *const T) -> Option<Self> {
    NonNull::new(ptr as *mut _).map(|nn| unsafe { Self::from_non_null(nn) })
  }

  #[inline(always)]
  pub(crate) unsafe fn from_raw_unchecked(ptr: *const T) -> Self {
    Self(
      unsafe { NonNull::new_unchecked(ptr as *mut _) },
      PhantomData,
    )
  }

  #[inline(always)]
  pub(crate) unsafe fn from_non_null(nn: NonNull<T>) -> Self {
    Self(nn, PhantomData)
  }

  #[inline(always)]
  pub(crate) fn as_non_null(self) -> NonNull<T> {
    self.0
  }

  #[inline(always)]
  pub(crate) fn slice_into_raw(slice: &[Self]) -> &[*const T] {
    unsafe { &*(slice as *const [Self] as *const [*const T]) }
  }
}

impl<T> Copy for Local<'_, T> {}

impl<T> Clone for Local<'_, T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Deref for Local<'_, T> {
  type Target = T;
  fn deref(&self) -> &T {
    unsafe { self.0.as_ref() }
  }
}
