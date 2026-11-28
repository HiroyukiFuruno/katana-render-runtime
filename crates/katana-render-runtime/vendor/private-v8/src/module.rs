use crate::private_v8::Module;
use crate::private_v8::support::int;
use std::num::NonZeroI32;

unsafe extern "C" {
  fn v8__Module__GetIdentityHash(this: *const Module) -> int;
}

impl Module {
  /// Returns the V8 hash value for this value. The current implementation
  /// uses a hidden property to store the identity hash.
  ///
  /// The return value will never be 0. Also, it is not guaranteed to be
  /// unique.
  #[inline(always)]
  pub fn get_identity_hash(&self) -> NonZeroI32 {
    unsafe { NonZeroI32::new_unchecked(v8__Module__GetIdentityHash(self)) }
  }

}
