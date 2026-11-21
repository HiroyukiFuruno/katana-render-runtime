use super::ffi;

impl super::SimdUtf {
/* --------------------------------------------------------------------------- */
/* Validation */
/* --------------------------------------------------------------------------- */



/// Validates that the input is valid ASCII.
#[inline]
pub fn validate_ascii(input: &[u8]) -> bool {
  unsafe { ffi::simdutf__validate_ascii(input.as_ptr(), input.len()) }
}







}
