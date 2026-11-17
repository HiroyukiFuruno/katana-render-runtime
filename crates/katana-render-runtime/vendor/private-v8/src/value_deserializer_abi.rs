use crate::private_v8::support::CxxVTable;

/* Must be == sizeof(v8::ValueDeserializer), */
/* see v8__ValueDeserializer__CONSTRUCT(). */
#[repr(C)]
pub struct CxxValueDeserializer {
  pub(super) _cxx_vtable: CxxVTable,
}

unsafe extern "C" {
  pub(super) fn v8__ValueDeserializer__DESTRUCT(
    this: *mut CxxValueDeserializer,
  );
}
