//! SIMD-accelerated Unicode validation and transcoding.
//!
//! These bindings expose the [simdutf](https://github.com/simdutf/simdutf)
//! library that is already bundled with V8, avoiding C++ symbol clashes
//! that occur when linking a separate simdutf crate.

mod conversion_latin1;
mod conversion_utf16;
mod ffi;
mod length;
mod types;
mod validation;

pub use types::{SimdUtf, SimdUtfResult};
