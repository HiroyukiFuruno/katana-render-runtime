use super::{Isolate, v8__Isolate__PerformMicrotaskCheckpoint};

impl Isolate {
  /// Runs the default MicrotaskQueue until it gets empty and perform other
  /// microtask checkpoint steps, such as calling ClearKeptObjects. Asserts that
  /// the MicrotasksPolicy is not kScoped. Any exceptions thrown by microtask
  /// callbacks are swallowed.
  #[inline(always)]
  pub fn perform_microtask_checkpoint(&mut self) {
    unsafe { v8__Isolate__PerformMicrotaskCheckpoint(self.as_real_ptr()) }
  }
}
