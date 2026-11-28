use super::{ArrayBufferAllocator, CounterLookupCallback};
use crate::private_v8::cppgc::Heap;
use crate::private_v8::support::{Opaque, SharedPtr, char, int, intptr_t};
use std::mem::{MaybeUninit, size_of};

#[path = "isolate_create_params_raw_limits.rs"]
mod limits;
pub(crate) use limits::ResourceConstraints;

#[repr(C)]
#[derive(Debug)]
pub(crate) struct RawStartupData {
  data: *const char,
  raw_size: int,
}

#[repr(C)]
#[derive(Debug)]
pub(crate) struct CreateParams {
  pub code_event_handler: *const Opaque, // JitCodeEventHandler
  pub constraints: ResourceConstraints,
  pub snapshot_blob: *const RawStartupData,
  pub counter_lookup_callback: Option<CounterLookupCallback>,
  pub create_histogram_callback: *const Opaque, // CreateHistogramCallback
  pub add_histogram_sample_callback: *const Opaque, // AddHistogramSampleCallback
  pub array_buffer_allocator: *mut ArrayBufferAllocator,
  pub array_buffer_allocator_shared: SharedPtr<ArrayBufferAllocator>,
  pub external_references: *const intptr_t,
  pub allow_atomics_wait: bool,
  _fatal_error_handler: *const Opaque, // FatalErrorCallback
  _oom_error_handler: *const Opaque,   // OOMErrorCallback
  pub cpp_heap: *const Heap,
}

unsafe extern "C" {
  fn v8__Isolate__CreateParams__CONSTRUCT(buf: *mut MaybeUninit<CreateParams>);
  fn v8__Isolate__CreateParams__SIZEOF() -> usize;
}

impl Default for CreateParams {
  fn default() -> Self {
    let size = unsafe { v8__Isolate__CreateParams__SIZEOF() };
    assert_eq!(size, size_of::<Self>());
    let mut buf = MaybeUninit::<Self>::uninit();
    unsafe { v8__Isolate__CreateParams__CONSTRUCT(&mut buf) };
    unsafe { buf.assume_init() }
  }
}
