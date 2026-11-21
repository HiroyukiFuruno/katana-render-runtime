
/// Minimum one-byte string length before the simdutf `utf8_length_from_latin1`
/// path beats std's inline `is_ascii` SWAR scan (the simdutf FFI call has fixed
/// overhead that only pays off once the scan is long enough).
pub(super) const ONEBYTE_SIMD_THRESHOLD: usize = 128;

/// Transcodes Latin-1 `bytes` into the caller-provided output region, returning
/// the number of UTF-8 bytes written (the UTF-8 length of `bytes`).
///
/// Callers write into uninitialized memory — a fresh `Vec`'s spare capacity or
/// a `MaybeUninit` borrow buffer — so the destination is passed as a raw
/// pointer + length rather than an already-initialized `&mut [u8]`. Centralizes
/// the one unsafe `simdutf` FFI call shared by the one-byte read paths.
///
/// # Safety
/// `out_ptr` must be valid for writes of `out_len` bytes, and `out_len` must be
/// at least the UTF-8 length of `bytes` (which never exceeds `bytes.len() * 2`).
#[inline(always)]
pub(super) unsafe fn transcode_latin1_to_utf8(
  bytes: &[u8],
  out_ptr: *mut u8,
  out_len: usize,
) -> usize {
  /* SAFETY: the caller guarantees `out_ptr` is valid for `out_len` writes. */
  let out = unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) };
  /* SAFETY: `out` covers the full UTF-8 expansion, so simdutf's write stays in */
  /* bounds; it always produces valid UTF-8 from Latin-1 input. */
  unsafe {
    crate::private_v8::simdutf::SimdUtf::convert_latin1_to_utf8(bytes, out)
  }
}

const ASCII_PREFIX_SCAN_BYTES: usize = 32;

/// Whether one-byte string data is pure ASCII. Uses simdutf's wide SIMD scan
/// for long strings (where it beats std's SWAR `is_ascii`) and the inline
/// `is_ascii` for short ones (avoiding the simdutf FFI-call overhead). Shared
/// by the one-byte read paths that only need the ASCII/Latin-1 decision.
#[inline(always)]
pub(super) fn onebyte_is_ascii(bytes: &[u8]) -> bool {
  if bytes.len() >= ONEBYTE_SIMD_THRESHOLD {
    /* simdutf's `validate_ascii` scans the *whole* buffer even when the very */
    /* first byte is non-ASCII, whereas std's `is_ascii` short-circuits. Do a */
    /* cheap inline early-reject on the head first so Latin-1 / non-ASCII text */
    /* (which typically has a high byte early) doesn't pay for a full SIMD scan */
    /* just to be rejected. Pure ASCII passes the head and then gets simdutf's */
    /* fast wide scan over the rest. */
    let head = bytes.len().min(ASCII_PREFIX_SCAN_BYTES);
    if !bytes[..head].is_ascii() {
      return false;
    }
    /* The head is already confirmed ASCII; scan only the remainder (ASCII-ness */
    /* is per-byte, so this is equivalent to validating the whole buffer). */
    return crate::private_v8::simdutf::SimdUtf::validate_ascii(&bytes[head..]);
  }
  bytes.is_ascii()
}

/// Converts one-byte (Latin-1) string data to an owned
/// [`std::string::String`].
///
/// With `simdutf`, a single `utf8_length_from_latin1` SIMD pass both detects
/// pure ASCII (result == input length) and yields the exact UTF-8 length for
/// the Latin-1 case, so an ASCII string is one SIMD scan + a memcpy and a
/// Latin-1 string is one SIMD scan + one SIMD transcode (down from the previous
/// `is_ascii` scan + separate length scan + transcode).
#[inline(always)]
pub(super) fn onebyte_to_string(bytes: &[u8]) -> std::string::String {
  const ONEBYTE_FUSE_THRESHOLD: usize = 4096;
  /* WHY: Large inputs amortize a worst-case allocation by avoiding a second scan. */
  if bytes.len() >= ONEBYTE_FUSE_THRESHOLD {
    return latin1_owned(bytes, bytes.len().saturating_mul(2));
  }
  if bytes.len() >= ONEBYTE_SIMD_THRESHOLD {
    let length =
      crate::private_v8::simdutf::SimdUtf::utf8_length_from_latin1(bytes);
    if length == bytes.len() {
      return ascii_owned(bytes);
    }
    let text = latin1_owned(bytes, length);
    debug_assert_eq!(text.len(), length);
    return text;
  }
  if bytes.is_ascii() {
    ascii_owned(bytes)
  } else {
    latin1_to_string(bytes)
  }
}

#[inline]
fn ascii_owned(bytes: &[u8]) -> std::string::String {
  debug_assert!(bytes.is_ascii());
  /* SAFETY: Callers check ASCII before conversion. */
  unsafe { std::str::from_utf8_unchecked(bytes) }.to_owned()
}

/// Converts Latin-1 bytes to an owned UTF-8 string using the exact allocation size.
#[inline]
pub(super) fn latin1_to_string(bytes: &[u8]) -> std::string::String {
  debug_assert!(!bytes.is_ascii());
  let length =
    crate::private_v8::simdutf::SimdUtf::utf8_length_from_latin1(bytes);
  let text = latin1_owned(bytes, length);
  debug_assert_eq!(text.len(), length);
  text
}

#[inline]
fn latin1_owned(bytes: &[u8], capacity: usize) -> std::string::String {
  let mut buf: Vec<u8> = Vec::with_capacity(capacity);
  /* SAFETY: The capacity covers the output and this encoder writes valid UTF-8. */
  unsafe {
    let written = transcode_latin1_to_utf8(bytes, buf.as_mut_ptr(), capacity);
    buf.set_len(written);
    std::string::String::from_utf8_unchecked(buf)
  }
}
