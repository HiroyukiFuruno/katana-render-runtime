use crate::private_v8::support::CxxVTable;

/* Must be == sizeof(v8::ValueSerializer), see v8__ValueSerializer__CONSTRUCT(). */
#[repr(C)]
pub struct CxxValueSerializer {
  pub(super) _cxx_vtable: CxxVTable,
}

unsafe extern "C" {
  pub(super) fn v8__ValueSerializer__DESTRUCT(this: *mut CxxValueSerializer);
}
