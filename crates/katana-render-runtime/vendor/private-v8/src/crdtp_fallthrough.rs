use std::ffi::c_void;

pub(super) type FallthroughCallback = Box<dyn FnMut(i32, &[u8], &[u8], &[u8])>;

pub(super) struct FallthroughCallbackData {
  pub(super) callback: FallthroughCallback,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn crdtp__FallthroughCallback__Run(
  callback: *mut c_void,
  call_id: i32,
  method_data: *const u8,
  method_len: usize,
  message_data: *const u8,
  message_len: usize,
  associated_data: *const u8,
  associated_data_len: usize,
) {
  unsafe {
    unsafe fn as_slice<'a>(data: *const u8, len: usize) -> &'a [u8] {
      if len == 0 {
        &[]
      } else {
        unsafe { std::slice::from_raw_parts(data, len) }
      }
    }

    let callback = &mut *(callback as *mut FallthroughCallbackData);
    (callback.callback)(
      call_id,
      as_slice(method_data, method_len),
      as_slice(message_data, message_len),
      as_slice(associated_data, associated_data_len),
    );
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn crdtp__FallthroughCallback__Drop(callback: *mut c_void) {
  unsafe {
    drop(Box::from_raw(callback as *mut FallthroughCallbackData));
  }
}
