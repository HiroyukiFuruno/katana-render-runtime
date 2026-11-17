use std::ptr::NonNull;

use super::Data;

/// A unique obligation to reset one V8 global-handle cell.
///
/// The cell is stable until it is reset or its isolate is disposed. Moving the
/// pointer between threads is safe, but resetting it is only safe while the
/// destination is the isolate home thread.
pub(super) struct DeferredGlobalReset(pub(super) NonNull<Data>);

/* SAFETY: This token is only created by `Global::drop`; that `Global` uniquely */
/* owns the cell and cannot use it again. The token is moved through a */
/* mutex-protected queue and consumed only by the isolate home thread. */
unsafe impl Send for DeferredGlobalReset {}

impl DeferredGlobalReset {
  pub(super) fn reset(self) {
    unsafe { super::v8__Global__Reset(self.0.as_ptr()) };
  }
}
