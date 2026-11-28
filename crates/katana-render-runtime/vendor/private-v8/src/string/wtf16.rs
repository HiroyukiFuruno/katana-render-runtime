
/* --------------------------------------------------------------------------- */
/* String conversion helpers. */
/* Hot paths dispatch to SIMD-accelerated routines in `crate::private_v8::simdutf`. */
/* --------------------------------------------------------------------------- */

/// The minimum number of UTF-16 code units before we try the SIMD path.
/// With the single-pass `convert_utf16le_to_utf8_with_errors` conversion the
/// crossover against the scalar `decode_utf16` loop is low; measured wins start
/// around 16 units.
const WTF16_SIMD_THRESHOLD: usize = 16;
const MAX_UTF8_BYTES_PER_UTF16_UNIT: usize = 3;

/// Converts (potentially ill-formed) UTF-16LE / WTF-16 code units to an
/// owned [`std::string::String`], replacing unpaired surrogates with U+FFFD.
#[inline(always)]
pub(super) fn wtf16_to_string(units: &[u16]) -> std::string::String {
  {
    /* Single simdutf pass that validates *and* converts. Each UTF-16 code unit */
    /* yields at most 3 UTF-8 bytes (surrogate pairs are 2 units -> 4 bytes), so */
    /* `len * 3` is a safe upper bound. On a lone-surrogate error we fall */
    /* through to the scalar WTF-16 loop below. */
    if units.len() >= WTF16_SIMD_THRESHOLD {
      let cap = units.len() * MAX_UTF8_BYTES_PER_UTF16_UNIT;
      let mut buf: Vec<u8> = Vec::with_capacity(cap);
      /* SAFETY: `buf` has `cap` bytes of spare capacity. */
      let result = unsafe {
        let out = std::slice::from_raw_parts_mut(buf.as_mut_ptr(), cap);
        crate::private_v8::simdutf::SimdUtf::convert_utf16le_to_utf8_with_errors(units, out)
      };
      if result.is_ok() {
        /* SAFETY: simdutf wrote `result.count` valid UTF-8 bytes. */
        unsafe {
          buf.set_len(result.count);
          return std::string::String::from_utf8_unchecked(buf);
        }
      }
    }
  }
  /* Scalar fallback: handles short strings and strings with unpaired */
  /* surrogates (WTF-16). */
  let mut buf = std::string::String::with_capacity(units.len() * MAX_UTF8_BYTES_PER_UTF16_UNIT);
  for result in std::char::decode_utf16(units.iter().copied()) {
    buf.push(result.unwrap_or('\u{FFFD}'));
  }
  buf
}
