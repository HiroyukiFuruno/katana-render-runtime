use super::NewStringType;
use super::ffi::{
  v8__String__Empty, v8__String__NewFromOneByte, v8__String__NewFromTwoByte,
  v8__String__NewFromUtf8,
};
use super::latin1::onebyte_is_ascii;
use crate::private_v8::Local;
use crate::private_v8::String;
use crate::private_v8::binding::v8__String__kMaxLength;
use crate::private_v8::scope::PinScope;
use crate::private_v8::simdutf::{SimdUtf, SimdUtfResult};
use crate::private_v8::support::char;

/// Minimum non-ASCII UTF-8 byte length before `new_from_utf8` decodes with
/// simdutf instead of V8's decoder. Below this the two potential simdutf FFI
/// calls cost more than V8 handling the tiny string itself.
const NONASCII_ENCODE_SIMD_THRESHOLD: usize = 16;

impl String {
  /// The maximum length (in bytes) of a buffer that a v8::String can be built
  /// from. Attempting to create a v8::String from a larger buffer will result
  /// in None being returned.
  pub const MAX_LENGTH: usize = v8__String__kMaxLength as _;

  #[inline(always)]
  pub fn empty<'s>(scope: &PinScope<'s, '_, ()>) -> Local<'s, String> {
    /* FIXME(bnoordhuis) v8__String__Empty() is infallible so there */
    /* is no need to box up the result, only to unwrap it again. */
    unsafe { scope.cast_local(|sd| v8__String__Empty(sd.get_isolate_ptr())) }.unwrap()
  }

  /// Allocates a new string from UTF-8 data. Only returns an empty value when
  /// length > kMaxLength
  #[inline(always)]
  pub fn new_from_utf8<'s>(
    scope: &PinScope<'s, '_, ()>,
    buffer: &[u8],
    new_type: NewStringType,
  ) -> Option<Local<'s, String>> {
    if buffer.is_empty() {
      return Some(Self::empty(scope));
    }
    /* WHY: V8 checks UTF-8 byte length before decoding. Keep that check
     * even when the decoded string would be shorter. */
    if buffer.len() > Self::MAX_LENGTH {
      return Self::new_from_utf8_raw(scope, buffer, new_type);
    }
    if onebyte_is_ascii(buffer) {
      return Self::new_from_one_byte(scope, buffer, new_type);
    }
    if buffer.len() >= NONASCII_ENCODE_SIMD_THRESHOLD {
      return Self::new_from_utf8_transcode(scope, buffer, new_type);
    }
    Self::new_from_utf8_raw(scope, buffer, new_type)
  }

  fn new_from_utf8_transcode<'s>(
    scope: &PinScope<'s, '_, ()>,
    buffer: &[u8],
    new_type: NewStringType,
  ) -> Option<Local<'s, String>> {
    if let Some(latin1) = decode_utf8(buffer, SimdUtf::convert_utf8_to_latin1_with_errors) {
      return Self::new_from_one_byte(scope, &latin1, new_type);
    }
    if let Some(utf16) = decode_utf8(buffer, SimdUtf::convert_utf8_to_utf16le_with_errors) {
      return Self::new_from_two_byte(scope, &utf16, new_type);
    }
    Self::new_from_utf8_raw(scope, buffer, new_type)
  }

  fn new_from_utf8_raw<'s>(
    scope: &PinScope<'s, '_, ()>,
    buffer: &[u8],
    new_type: NewStringType,
  ) -> Option<Local<'s, String>> {
    let buffer_len = buffer.len().try_into().ok()?;
    unsafe {
      scope.cast_local(|sd| {
        v8__String__NewFromUtf8(
          sd.get_isolate_ptr(),
          buffer.as_ptr() as *const char,
          new_type,
          buffer_len,
        )
      })
    }
  }

  /// Allocates a new string from Latin-1 data.  Only returns an empty value when
  /// length > kMaxLength.
  #[inline(always)]
  pub fn new_from_one_byte<'s>(
    scope: &PinScope<'s, '_, ()>,
    buffer: &[u8],
    new_type: NewStringType,
  ) -> Option<Local<'s, String>> {
    let buffer_len = buffer.len().try_into().ok()?;
    unsafe {
      scope.cast_local(|sd| v8__String__NewFromOneByte(sd.get_isolate_ptr(), buffer.as_ptr(), new_type, buffer_len))
    }
  }

  /// Allocates a new string from UTF-16 data. Only returns an empty value when
  /// length > kMaxLength.
  #[inline(always)]
  pub fn new_from_two_byte<'s>(
    scope: &PinScope<'s, '_, ()>,
    buffer: &[u16],
    new_type: NewStringType,
  ) -> Option<Local<'s, String>> {
    let buffer_len = buffer.len().try_into().ok()?;
    unsafe {
      scope.cast_local(|sd| v8__String__NewFromTwoByte(sd.get_isolate_ptr(), buffer.as_ptr(), new_type, buffer_len))
    }
  }
  /* Convenience function not present in the original V8 API. */
  #[inline(always)]
  pub fn new<'s>(scope: &PinScope<'s, '_, ()>, value: &str) -> Option<Local<'s, String>> {
    Self::new_from_utf8(scope, value.as_ref(), NewStringType::Normal)
  }
}

fn decode_utf8<T>(buffer: &[u8], convert: unsafe fn(&[u8], &mut [T]) -> SimdUtfResult) -> Option<Vec<T>> {
  let mut output: Vec<T> = Vec::with_capacity(buffer.len());
  /* SAFETY: Both decoders only write and output at most one unit for
   * each input byte. The returned count is the initialized prefix. */
  let converted = unsafe {
    let units = std::slice::from_raw_parts_mut(output.as_mut_ptr(), buffer.len());
    convert(buffer, units)
  };
  if !converted.is_ok() {
    return None;
  }
  unsafe { output.set_len(converted.count) };
  Some(output)
}
