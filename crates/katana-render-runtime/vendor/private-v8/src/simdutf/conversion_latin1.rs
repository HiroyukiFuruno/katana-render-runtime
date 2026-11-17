use super::{ffi, SimdUtfResult};

impl super::SimdUtf {
/* --------------------------------------------------------------------------- */
/* Conversion: UTF-8 <-> Latin-1 */
/* --------------------------------------------------------------------------- */


/// Converts UTF-8 to Latin-1 with error reporting.
///
/// # Safety
///
/// `output` must have at least `input.len()` bytes of capacity.
#[inline]
pub unsafe fn convert_utf8_to_latin1_with_errors(
  input: &[u8],
  output: &mut [u8],
) -> SimdUtfResult {
  SimdUtfResult::from_ffi(unsafe {
    ffi::simdutf__convert_utf8_to_latin1_with_errors(
      input.as_ptr(),
      input.len(),
      output.as_mut_ptr(),
    )
  })
}


/// Converts Latin-1 to UTF-8.
///
/// # Safety
///
/// `output` must have at least `input.len() * 2` bytes of capacity.
/// Use [`utf8_length_from_latin1`] for an exact count.
#[inline]
pub unsafe fn convert_latin1_to_utf8(input: &[u8], output: &mut [u8]) -> usize {
  unsafe {
    ffi::simdutf__convert_latin1_to_utf8(
      input.as_ptr(),
      input.len(),
      output.as_mut_ptr(),
    )
  }
}

/* --------------------------------------------------------------------------- */
/* Conversion: Latin-1 <-> UTF-16LE */
/* --------------------------------------------------------------------------- */


}
