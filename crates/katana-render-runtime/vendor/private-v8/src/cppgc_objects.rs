use super::*;

/// Base trait for objects supporting garbage collection.
///
/// # Safety
///
/// Implementors must visit every managed pointer reachable from the object.
pub unsafe trait GarbageCollected {
  /// Visits managed pointers reachable from this object.
  fn trace(&self, visitor: &mut Visitor);

  /// Specifies a name for the garbage-collected object. Such names will never
  /// be hidden, as they are explicitly specified by the user of this API.
  ///
  /// V8 may call this function while generating a heap snapshot or at other
  /// times. If V8 is currently generating a heap snapshot (according to
  /// HeapProfiler::IsTakingSnapshot), then the returned string must stay alive
  /// until the snapshot generation has completed. Otherwise, the returned string
  /// must stay alive forever. If you need a place to store a temporary string
  /// during snapshot generation, use HeapProfiler::CopyNameForHeapSnapshot.
  fn get_name(&self) -> &'static CStr;
}

#[repr(C)]
struct RustObjConcrete<T> {
  head: RustObj,
  dynamic: *mut dyn GarbageCollected,
  value: T,
}

pub(super) unsafe fn get_rust_obj<'s>(obj: *const RustObj) -> &'s dyn GarbageCollected {
  unsafe {
    let obj = &*(obj as *const RustObjConcrete<()>);
    &*obj.dynamic
  }
}

pub(super) unsafe fn get_rust_obj_mut<'s>(
  obj: *mut RustObj,
) -> &'s mut dyn GarbageCollected {
  unsafe {
    let obj = &mut *(obj as *mut RustObjConcrete<()>);
    &mut *obj.dynamic
  }
}
