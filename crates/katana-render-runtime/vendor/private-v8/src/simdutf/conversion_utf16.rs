use super::{ffi, SimdUtfResult};

impl super::SimdUtf {
/* --------------------------------------------------------------------------- */
/* Conversion: UTF-8 <-> UTF-16LE */
/* --------------------------------------------------------------------------- */


/// Converts UTF-8 to UTF-16LE with error reporting.
///
/// # Safety
///
/// `output` must have at least `input.len()` elements of capacity.
#[inline]
pub unsafe fn convert_utf8_to_utf16le_with_errors(
  input: &[u8],
  output: &mut [u16],
) -> SimdUtfResult {
  SimdUtfResult::from_ffi(unsafe {
    ffi::simdutf__convert_utf8_to_utf16le_with_errors(
      input.as_ptr(),
      input.len(),
      output.as_mut_ptr(),
    )
  })
}



/// Converts UTF-16LE to UTF-8 with error reporting.
///
/// # Safety
///
/// `output` must have at least `input.len() * 3` bytes of capacity.
#[inline]
pub unsafe fn convert_utf16le_to_utf8_with_errors(
  input: &[u16],
  output: &mut [u8],
) -> SimdUtfResult {
  SimdUtfResult::from_ffi(unsafe {
    ffi::simdutf__convert_utf16le_to_utf8_with_errors(
      input.as_ptr(),
      input.len(),
      output.as_mut_ptr(),
    )
  })
}


/* --------------------------------------------------------------------------- */
/* Conversion: UTF-8 <-> UTF-16BE */
/* --------------------------------------------------------------------------- */


}
