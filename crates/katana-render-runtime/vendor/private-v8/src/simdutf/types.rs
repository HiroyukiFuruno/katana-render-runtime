use super::ffi::FfiResult;

/* WHY: Keep Rust's public repr values tied explicitly to simdutf's FFI codes. */
const ERROR_SUCCESS: i32 = 0;
const ERROR_HEADER_BITS: i32 = 1;
const ERROR_TOO_SHORT: i32 = 2;
const ERROR_TOO_LONG: i32 = 3;
const ERROR_OVERLONG: i32 = 4;
const ERROR_TOO_LARGE: i32 = 5;
const ERROR_SURROGATE: i32 = 6;
const ERROR_INVALID_BASE64_CHARACTER: i32 = 7;
const ERROR_BASE64_INPUT_REMAINDER: i32 = 8;
const ERROR_BASE64_EXTRA_BITS: i32 = 9;
const ERROR_OUTPUT_BUFFER_TOO_SMALL: i32 = 10;
const ERROR_OTHER: i32 = 11;

/// Error codes returned by simdutf operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ErrorCode {
  Success = ERROR_SUCCESS,
  HeaderBits = ERROR_HEADER_BITS,
  TooShort = ERROR_TOO_SHORT,
  TooLong = ERROR_TOO_LONG,
  Overlong = ERROR_OVERLONG,
  TooLarge = ERROR_TOO_LARGE,
  Surrogate = ERROR_SURROGATE,
  InvalidBase64Character = ERROR_INVALID_BASE64_CHARACTER,
  Base64InputRemainder = ERROR_BASE64_INPUT_REMAINDER,
  Base64ExtraBits = ERROR_BASE64_EXTRA_BITS,
  OutputBufferTooSmall = ERROR_OUTPUT_BUFFER_TOO_SMALL,
  Other = ERROR_OTHER,
}

impl ErrorCode {
  fn from_i32(v: i32) -> Self {
    match v {
      ERROR_SUCCESS => Self::Success,
      ERROR_HEADER_BITS => Self::HeaderBits,
      ERROR_TOO_SHORT => Self::TooShort,
      ERROR_TOO_LONG => Self::TooLong,
      ERROR_OVERLONG => Self::Overlong,
      ERROR_TOO_LARGE => Self::TooLarge,
      ERROR_SURROGATE => Self::Surrogate,
      ERROR_INVALID_BASE64_CHARACTER => Self::InvalidBase64Character,
      ERROR_BASE64_INPUT_REMAINDER => Self::Base64InputRemainder,
      ERROR_BASE64_EXTRA_BITS => Self::Base64ExtraBits,
      ERROR_OUTPUT_BUFFER_TOO_SMALL => Self::OutputBufferTooSmall,
      _ => Self::Other,
    }
  }
}

/// Namespace for the bundled SIMD Unicode operations.
pub struct SimdUtf;

/// Result of a simdutf operation.
///
/// On success, `count` is the number of code units written/validated.
/// On error, `count` is the position of the error in the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimdUtfResult {
  pub error: ErrorCode,
  pub count: usize,
}

impl SimdUtfResult {
  pub(super) fn from_ffi(r: FfiResult) -> Self {
    Self {
      error: ErrorCode::from_i32(r.error),
      count: r.count,
    }
  }

  /// Returns `true` if the operation succeeded.
  #[inline]
  pub fn is_ok(&self) -> bool {
    self.error == ErrorCode::Success
  }
}
