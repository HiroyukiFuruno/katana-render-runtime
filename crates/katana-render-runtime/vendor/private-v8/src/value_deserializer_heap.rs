use super::*;

/* Must be == sizeof(v8::ValueDeserializer::Delegate), */
/* see v8__ValueDeserializer__Delegate__CONSTRUCT(). */
#[repr(C)]
pub struct CxxValueDeserializerDelegate {
  pub(super) _cxx_vtable: CxxVTable,
}

/// The ValueDeserializerHeap object contains all objects related to a
/// deserializer. This object has to be pinned to the heap because of the Cpp
/// pointers that have to remain valid. Moving this object would result in the
/// Cpp pointer to the delegate to become invalid and thus causing the delegate
/// callback to fail. Additionally the deserializer and implementation are also
/// pinned in memory because these have to be accessable from within the
/// delegate callback methods.
pub struct ValueDeserializerHeap<'a> {
  pub(super) value_deserializer_impl: Box<dyn ValueDeserializerImpl + 'a>,
  pub(super) cxx_value_deserializer: CxxValueDeserializer,
  pub(super) cxx_value_deserializer_delegate: CxxValueDeserializerDelegate,
  pub(super) context: Global<Context>,
}

impl ValueDeserializerHeap<'_> {
  fn get_cxx_value_deserializer_delegate_offset()
  -> FieldOffset<CxxValueDeserializerDelegate> {
    let buf = std::mem::MaybeUninit::<Self>::uninit();
    let delegate =
      unsafe { addr_of!((*buf.as_ptr()).cxx_value_deserializer_delegate) };
    FieldOffset::from_ptrs(buf.as_ptr(), delegate)
  }

  /// Starting from 'this' pointer a ValueDeserializerHeap ref can be created
  pub unsafe fn dispatch(
    value_serializer_delegate: &CxxValueDeserializerDelegate,
  ) -> &Self {
    unsafe {
      Self::get_cxx_value_deserializer_delegate_offset()
        .to_embedder::<Self>(value_serializer_delegate)
    }
  }
}

impl Drop for ValueDeserializerHeap<'_> {
  fn drop(&mut self) {
    unsafe {
      v8__ValueDeserializer__DESTRUCT(&mut self.cxx_value_deserializer);
    };
  }
}

#[path = "value_deserializer_callbacks.rs"]
mod callbacks;
