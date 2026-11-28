/* --------------------------------------------------------------------------- */
/* FFI declarations */
/* --------------------------------------------------------------------------- */

#[repr(C)]
pub(super) struct FfiResult {
  pub(super) error: i32,
  pub(super) count: usize,
}

unsafe extern "C" {
  /* Validation */
  pub(super) fn simdutf__validate_ascii(buf: *const u8, len: usize) -> bool;

  /* Conversion: UTF-8 <-> UTF-16LE */
  pub(super) fn simdutf__convert_utf8_to_utf16le_with_errors(
    input: *const u8,
    length: usize,
    output: *mut u16,
  ) -> FfiResult;
  pub(super) fn simdutf__convert_utf16le_to_utf8_with_errors(
    input: *const u16,
    length: usize,
    output: *mut u8,
  ) -> FfiResult;

  /* Conversion: UTF-8 <-> UTF-16BE */

  /* Conversion: UTF-8 <-> Latin-1 */
  pub(super) fn simdutf__convert_utf8_to_latin1_with_errors(
    input: *const u8,
    length: usize,
    output: *mut u8,
  ) -> FfiResult;
  pub(super) fn simdutf__convert_latin1_to_utf8(
    input: *const u8,
    length: usize,
    output: *mut u8,
  ) -> usize;

  /* Conversion: Latin-1 <-> UTF-16LE */

  /* Conversion: UTF-8 <-> UTF-32 */

  /* Length calculation */
  pub(super) fn simdutf__utf8_length_from_latin1(input: *const u8, length: usize)
  -> usize;

  /* Counting */

  /* Encoding detection */

  /* Base64 */
}
