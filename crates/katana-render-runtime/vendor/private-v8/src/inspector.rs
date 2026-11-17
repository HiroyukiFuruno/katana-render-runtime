/* Copyright 2019-2021 the Deno authors. All rights reserved. MIT license. */

//! Bindings to the V8 Inspector API. Documentation for the V8 inspector API is
//! very sparse, so here are a few references for the next sorry soul who has to
//! dig into it.
//!
//! https://medium.com/@hyperandroid/v8-inspector-from-an-embedder-standpoint-7f9c0472e2b7
//! https://v8.dev/docs/inspector
//! https://chromedevtools.github.io/debugger-protocol-viewer/tot/
//! https://cs.chromium.org/chromium/src/v8/include/v8-inspector.h
//! https://github.com/nodejs/node/blob/v13.7.0/src/inspector_agent.cc
//! https://github.com/nodejs/node/blob/v13.7.0/src/inspector_agent.h
//! https://github.com/nodejs/node/tree/v13.7.0/src/inspector
//! https://github.com/denoland/deno/blob/v0.38.0/cli/inspector.rs

use crate::private_v8::Context;
use crate::private_v8::Local;
use crate::private_v8::Value;
use crate::private_v8::support::CxxVTable;
use crate::private_v8::support::UniquePtr;
use crate::private_v8::support::int;
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::fmt::{self, Debug};
use std::iter::ExactSizeIterator;
use std::iter::IntoIterator;
use std::marker::PhantomPinned;
#[cfg(test)]
use std::mem::MaybeUninit;
use std::pin::pin;

#[path = "inspector_channel.rs"]
mod inspector_channel;
#[path = "inspector_client.rs"]
mod inspector_client;
#[path = "inspector_client_callbacks.rs"]
mod inspector_client_callbacks;
#[path = "inspector_ffi.rs"]
mod inspector_ffi;
#[path = "inspector_inspectable.rs"]
mod inspector_inspectable;
#[path = "inspector_stack_trace.rs"]
mod inspector_stack_trace;
#[path = "inspector_string_buffer.rs"]
mod inspector_string_buffer;
#[path = "inspector_string_view.rs"]
mod inspector_string_view;
#[path = "inspector_string_view_display.rs"]
mod inspector_string_view_display;
#[path = "inspector_string_view_iter.rs"]
mod inspector_string_view_iter;

pub use inspector_client::RawV8InspectorClient;
pub use inspector_stack_trace::V8StackTrace;
pub use inspector_string_buffer::StringBuffer;
pub use inspector_string_view::StringView;

#[cfg(test)]
use inspector_channel::RawChannel;

#[cfg(test)]
mod tests {
  use super::inspector_channel::ChannelImpl;
  use super::*;
  use crate::private_v8::support::UniquePtr;
  use std::sync::atomic::AtomicUsize;
  use std::sync::atomic::Ordering::SeqCst;

  static MESSAGE: &[u8] = b"Hello Pluto!";
  static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

  #[derive(Debug)]
  pub struct TestChannel {
    field1: i32,
    field2: u64,
  }

  impl ChannelImpl for TestChannel {
    fn send_response(
      &self,
      call_id: i32,
      mut message: UniquePtr<StringBuffer>,
    ) {
      assert_eq!(call_id, 999);
      assert_eq!(message.as_mut().unwrap().string().len(), MESSAGE.len());
      self.log_call();
    }
    fn send_notification(&self, mut message: UniquePtr<StringBuffer>) {
      assert_eq!(message.as_mut().unwrap().string().len(), MESSAGE.len());
      self.log_call();
    }
    fn flush_protocol_notifications(&self) {
      self.log_call();
    }
  }

  impl TestChannel {
    pub fn new() -> Self {
      Self {
        field1: -42,
        field2: 420,
      }
    }

    fn log_call(&self) {
      assert_eq!(self.field1, -42);
      assert_eq!(self.field2, 420);
      CALL_COUNT.fetch_add(1, SeqCst);
    }
  }

  #[test]
  fn test_channel() {
    let msg_view = StringView::from(MESSAGE);
    super::inspector_channel::exercise_callbacks(Box::new(TestChannel::new()), msg_view);
    assert_eq!(CALL_COUNT.swap(0, SeqCst), 3);
  }
}
