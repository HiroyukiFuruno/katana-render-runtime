use super::ffi;

impl super::SimdUtf {
/* --------------------------------------------------------------------------- */
/* Length calculation */
/* --------------------------------------------------------------------------- */




/// Returns the number of UTF-8 bytes needed to encode the given Latin-1 data.
#[inline]
pub fn utf8_length_from_latin1(input: &[u8]) -> usize {
  unsafe { ffi::simdutf__utf8_length_from_latin1(input.as_ptr(), input.len()) }
}





}
