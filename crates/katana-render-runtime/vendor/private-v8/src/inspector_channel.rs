use super::*;
#[cfg(test)]
use super::inspector_ffi::*;

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8Inspector__Channel__BASE__sendResponse(
  this: *mut RawChannel,
  call_id: int,
  message: UniquePtr<StringBuffer>,
) {
  unsafe {
    let channel = ChannelHeap::from_raw(this);
    channel.imp.send_response(call_id, message);
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8Inspector__Channel__BASE__sendNotification(
  this: *mut RawChannel,
  message: UniquePtr<StringBuffer>,
) {
  unsafe {
    ChannelHeap::from_raw(this).imp.send_notification(message);
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8Inspector__Channel__BASE__flushProtocolNotifications(
  this: *mut RawChannel,
) {
  unsafe {
    ChannelHeap::from_raw(this)
      .imp
      .flush_protocol_notifications();
  }
}

#[repr(C)]
#[derive(Debug)]
pub(super) struct RawChannel {
  _cxx_vtable: CxxVTable,
}

#[repr(C)]
struct ChannelHeap {
  raw: UnsafeCell<RawChannel>,
  imp: Box<dyn ChannelImpl>,
  _pinned: PhantomPinned,
}

impl ChannelHeap {
  unsafe fn from_raw<'b>(this: *const RawChannel) -> &'b ChannelHeap {
    unsafe { &(*this.cast::<ChannelHeap>()) }
  }
}

#[cfg(test)]
pub(super) fn exercise_callbacks(imp: Box<dyn ChannelImpl>, message: StringView<'_>) {
  let heap = Box::into_raw(Box::new(MaybeUninit::<ChannelHeap>::uninit()))
    .cast::<ChannelHeap>();
  unsafe {
    v8_inspector__V8Inspector__Channel__BASE__CONSTRUCT(
      (&raw mut (*heap).raw).cast(),
    );
    (&raw mut (*heap).imp).write(imp);
    (&raw mut (*heap)._pinned).write(PhantomPinned);
    let channel = (*heap).raw.get();
    v8_inspector__V8Inspector__Channel__BASE__sendResponse(
      channel,
      999,
      StringBuffer::create(message),
    );
    v8_inspector__V8Inspector__Channel__BASE__sendNotification(
      channel,
      StringBuffer::create(message),
    );
    v8_inspector__V8Inspector__Channel__BASE__flushProtocolNotifications(channel);
    drop(Box::from_raw(heap));
  }
}

pub trait ChannelImpl {
  fn send_response(&self, call_id: i32, message: UniquePtr<StringBuffer>);
  fn send_notification(&self, message: UniquePtr<StringBuffer>);
  fn flush_protocol_notifications(&self);
}
