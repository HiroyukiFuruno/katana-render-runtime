use super::*;

/* Must be == sizeof(v8::ValueSerializer::Delegate), */
/* see v8__ValueSerializer__Delegate__CONSTRUCT(). */
#[repr(C)]
pub struct CxxValueSerializerDelegate {
  pub(super) _cxx_vtable: CxxVTable,
}

/// The ValueSerializerHeap object contains all objects related to serializer.
/// This object has to be pinned to the heap because of the Cpp pointers that
/// have to remain valid. Moving this object would result in the Cpp pointer
/// to the delegate to become invalid and thus causing the delegate callback
/// to fail. Additionally the serializer and implementation are also pinned
/// in memory because these have to be accessable from within the delegate
/// callback methods.
pub struct ValueSerializerHeap<'a> {
  pub(super) value_serializer_impl: Box<dyn ValueSerializerImpl + 'a>,
  pub(super) cxx_value_serializer_delegate: CxxValueSerializerDelegate,
  pub(super) cxx_value_serializer: CxxValueSerializer,
  pub(super) buffer_size: AtomicUsize,
  pub(super) context: Global<Context>,
  pub(super) isolate_ptr: *mut RealIsolate,
}

impl ValueSerializerHeap<'_> {
  fn get_cxx_value_serializer_delegate_offset()
  -> FieldOffset<CxxValueSerializerDelegate> {
    let buf = std::mem::MaybeUninit::<Self>::uninit();
    let delegate =
      unsafe { addr_of!((*buf.as_ptr()).cxx_value_serializer_delegate) };
    FieldOffset::from_ptrs(buf.as_ptr(), delegate)
  }

  /// Starting from 'this' pointer a ValueSerializerHeap ref can be created
  pub unsafe fn dispatch(
    value_serializer_delegate: &CxxValueSerializerDelegate,
  ) -> &Self {
    unsafe {
      Self::get_cxx_value_serializer_delegate_offset()
        .to_embedder::<Self>(value_serializer_delegate)
    }
  }
}

impl Drop for ValueSerializerHeap<'_> {
  fn drop(&mut self) {
    unsafe { v8__ValueSerializer__DESTRUCT(&mut self.cxx_value_serializer) };
  }
}

#[path = "value_serializer_callbacks.rs"]
mod callbacks;
