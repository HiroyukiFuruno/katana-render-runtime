use super::*;
use crate::private_v8::support::CxxVTable;
use std::cell::UnsafeCell;

#[repr(C)]
pub(super) struct RawFrontendChannel {
  _cxx_vtable: CxxVTable,
}

/// Trait for sending protocol responses and notifications to clients.
pub trait FrontendChannelImpl {
  /// Send a response to a protocol request.
  fn send_protocol_response(&mut self, call_id: i32, message: Serializable);
  /// Send a notification (no call_id).
  fn send_protocol_notification(&mut self, message: Serializable);
  /// Flush any queued notifications.
  fn flush_protocol_notifications(&mut self);
}

/// Wraps a Rust `FrontendChannelImpl` for use with the C++ dispatcher.
pub struct FrontendChannel {
  raw: UnsafeCell<RawFrontendChannel>,
  imp: Box<dyn FrontendChannelImpl>,
}

impl FrontendChannel {
  unsafe fn from_raw<'a>(ptr: *mut RawFrontendChannel) -> &'a mut Self {
    unsafe {
      let channel_ptr = ptr as *mut u8;
      let offset = std::mem::offset_of!(FrontendChannel, raw);
      let self_ptr = channel_ptr.sub(offset) as *mut Self;
      &mut *self_ptr
    }
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn crdtp__FrontendChannel__BASE__sendProtocolResponse(
  this: *mut RawFrontendChannel,
  call_id: i32,
  message: *mut RawSerializable,
) {
  unsafe {
    let channel = FrontendChannel::from_raw(this);
    let msg = Serializable { ptr: message };
    channel.imp.send_protocol_response(call_id, msg);
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn crdtp__FrontendChannel__BASE__sendProtocolNotification(
  this: *mut RawFrontendChannel,
  message: *mut RawSerializable,
) {
  unsafe {
    let channel = FrontendChannel::from_raw(this);
    let msg = Serializable { ptr: message };
    channel.imp.send_protocol_notification(msg);
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn crdtp__FrontendChannel__BASE__flushProtocolNotifications(
  this: *mut RawFrontendChannel,
) {
  unsafe {
    let channel = FrontendChannel::from_raw(this);
    channel.imp.flush_protocol_notifications();
  }
}
