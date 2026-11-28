use super::ffi::{
  v8__String__ValueView__CONSTRUCT, v8__String__ValueView__DESTRUCT,
};
use crate::private_v8::Isolate;
use crate::private_v8::String;
use std::marker::PhantomData;

/* WHY: Match v8::String::ValueView in v8-primitive.h without accessor FFI calls. */
const PTR: usize = std::mem::size_of::<*const u8>();
const DATA_OFFSET: usize = PTR;
const LENGTH_OFFSET: usize = PTR + PTR;
const IS_ONE_BYTE_OFFSET: usize = LENGTH_OFFSET + std::mem::size_of::<u32>();

#[derive(Debug, PartialEq)]
pub enum ValueViewData<'s> {
  OneByte(&'s [u8]),
  TwoByte(&'s [u16]),
}

/// Returns a view onto a string's contents.
///
/// WARNING: This does not copy the string's contents, and will therefore be
/// invalidated if the GC can move the string while the ValueView is alive. It
/// is therefore required that no GC or allocation can happen while there is an
/// active ValueView. This requirement may be relaxed in the future.
///
/// V8 strings are either encoded as one-byte or two-bytes per character.
#[repr(C)]
pub struct ValueView<'s>(
  [u8; crate::private_v8::binding::v8__String__ValueView_SIZE],
  PhantomData<&'s ()>,
);

impl<'s> ValueView<'s> {
  /// Constructs a `ValueView` from a raw string reference.
  ///
  /// # Safety
  ///
  /// The caller must ensure that `string` is a valid V8 string that
  /// remains alive for at least `'s`. In practice this means the
  /// string must be reachable from a handle scope that outlives the
  /// returned `ValueView`.
  #[inline(always)]
  pub(crate) unsafe fn new_from_ref(
    isolate: &Isolate,
    string: &'s String,
  ) -> Self {
    let mut v = std::mem::MaybeUninit::uninit();
    unsafe {
      v8__String__ValueView__CONSTRUCT(
        v.as_mut_ptr(),
        isolate.as_real_ptr(),
        string,
      );
      v.assume_init()
    }
  }

  #[inline(always)]
  pub fn data(&self) -> ValueViewData<'_> {
    let (length, is_one_byte) = self.data_shape();
    if length == 0 {
      /* WHY: Empty V8 strings can carry a null data pointer. */
      return if is_one_byte {
        ValueViewData::OneByte(&[])
      } else {
        ValueViewData::TwoByte(&[])
      };
    }
    unsafe {
      let data = self
        .0
        .as_ptr()
        .add(DATA_OFFSET)
        .cast::<*const u8>()
        .read_unaligned();
      if is_one_byte {
        ValueViewData::OneByte(std::slice::from_raw_parts(data, length))
      } else {
        ValueViewData::TwoByte(std::slice::from_raw_parts(
          data.cast::<u16>(),
          length,
        ))
      }
    }
  }

  #[inline(always)]
  fn data_shape(&self) -> (usize, bool) {
    let base = self.0.as_ptr();
    unsafe {
      let length =
        base.add(LENGTH_OFFSET).cast::<u32>().read_unaligned() as usize;
      let is_one_byte = *base.add(IS_ONE_BYTE_OFFSET) != 0;
      (length, is_one_byte)
    }
  }


}

impl Drop for ValueView<'_> {
  fn drop(&mut self) {
    unsafe { v8__String__ValueView__DESTRUCT(self) }
  }
}
