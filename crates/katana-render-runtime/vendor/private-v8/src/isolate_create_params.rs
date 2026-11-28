use crate::private_v8::array_buffer::Allocator as ArrayBufferAllocator;
use crate::private_v8::support::SharedPtr;
use crate::private_v8::support::char;

#[path = "isolate_create_params_raw.rs"]
pub(crate) mod raw;

/// Should return a pointer to memory that persists for the lifetime of the
/// isolate.
pub type CounterLookupCallback =
  unsafe extern "C" fn(name: *const char) -> *mut i32;

/// Initial configuration parameters for a new Isolate.
#[must_use]
#[derive(Debug, Default)]
pub struct CreateParams {
  raw: raw::CreateParams,
}

impl CreateParams {
  /// The ArrayBuffer::ArrayBufferAllocator to use for allocating and freeing
  /// the backing store of ArrayBuffers.
  pub fn array_buffer_allocator(
    mut self,
    array_buffer_allocator: impl Into<SharedPtr<ArrayBufferAllocator>>,
  ) -> Self {
    self.raw.array_buffer_allocator_shared = array_buffer_allocator.into();
    self
  }

  pub(crate) fn finalize(mut self) -> raw::CreateParams {
    if self.raw.array_buffer_allocator_shared.is_null() {
      self = self.array_buffer_allocator(ArrayBufferAllocator::new_default());
    }
    self.raw
  }
}

#[test]
fn create_param_defaults() {
  let params = CreateParams::default();
  assert!(params.raw.allow_atomics_wait);
  assert!(params.raw.snapshot_blob.is_null());
  assert!(params.raw.external_references.is_null());
}
