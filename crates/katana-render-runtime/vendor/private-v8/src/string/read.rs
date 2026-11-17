use super::latin1::onebyte_to_string;
use super::wtf16::wtf16_to_string;
use super::{ValueView, ValueViewData};
use crate::private_v8::Isolate;
use crate::private_v8::String;

impl String {
  /// Creates a copy of a [`crate::private_v8::String`] in a [`std::string::String`].
  /// Convenience function not present in the original V8 API.
  ///
  /// Uses [`ValueView`] internally for single-pass access to the string
  /// data, with SIMD-accelerated transcoding for Latin-1 and two-byte
  /// strings.
  pub fn to_rust_string_lossy(&self, scope: &Isolate) -> std::string::String {
    /* No preliminary `self.length()` FFI call: the `ValueView` reports the */
    /* length, and `data()` yields an empty slice for empty strings, which the */
    /* ASCII arm below turns into an empty `String`. */
    /* SAFETY: `self` is a valid V8 string reachable from a handle scope. */
    let view = unsafe { ValueView::new_from_ref(scope, self) };

    match view.data() {
      ValueViewData::OneByte(bytes) => onebyte_to_string(bytes),
      ValueViewData::TwoByte(units) => wtf16_to_string(units),
    }
  }


}
