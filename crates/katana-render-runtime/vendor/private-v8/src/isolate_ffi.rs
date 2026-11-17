use super::*;

unsafe extern "C" {
  pub(super) fn v8__Isolate__New(params: *const raw::CreateParams) -> *mut RealIsolate;
  pub(super) fn v8__Isolate__Dispose(this: *mut RealIsolate);
  pub(super) fn v8__Isolate__GetNumberOfDataSlots(this: *const RealIsolate) -> u32;
  pub(super) fn v8__Isolate__GetData(
    isolate: *const RealIsolate,
    slot: u32,
  ) -> *mut c_void;
  pub(super) fn v8__Isolate__SetData(
    isolate: *const RealIsolate,
    slot: u32,
    data: *mut c_void,
  );
  pub(super) fn v8__Isolate__Enter(this: *mut RealIsolate);
  pub(super) fn v8__Isolate__Exit(this: *mut RealIsolate);
  pub(super) fn v8__Global__Reset(data: *const Data);
  pub(super) fn v8__Isolate__GetCurrent() -> *mut RealIsolate;
  pub(super) fn v8__Isolate__PerformMicrotaskCheckpoint(isolate: *mut RealIsolate);

}
