use crate::private_v8::String;
use crate::private_v8::isolate::RealIsolate;
use crate::private_v8::support::char;
use crate::private_v8::support::int;
use super::{NewStringType, ValueView};

unsafe extern "C" {
  pub(super) fn v8__String__Empty(isolate: *mut RealIsolate) -> *const String;

  pub(super) fn v8__String__NewFromUtf8(
    isolate: *mut RealIsolate,
    data: *const char,
    new_type: NewStringType,
    length: int,
  ) -> *const String;

  pub(super) fn v8__String__NewFromOneByte(
    isolate: *mut RealIsolate,
    data: *const u8,
    new_type: NewStringType,
    length: int,
  ) -> *const String;

  pub(super) fn v8__String__NewFromTwoByte(
    isolate: *mut RealIsolate,
    data: *const u16,
    new_type: NewStringType,
    length: int,
  ) -> *const String;

  pub(super) fn v8__String__ValueView__CONSTRUCT(
    buf: *mut ValueView,
    isolate: *mut RealIsolate,
    string: *const String,
  );
  pub(super) fn v8__String__ValueView__DESTRUCT(this: *mut ValueView);
}
