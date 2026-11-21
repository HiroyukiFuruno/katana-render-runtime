use super::{
  Isolate, v8__Isolate__Dispose, v8__Isolate__Enter, v8__Isolate__Exit,
};

impl Isolate {
  /// Sets this isolate as the entered one for the current thread.
  /// Saves the previously entered one (if any), so that it can be
  /// restored when exiting.  Re-entering an isolate is allowed.
  ///
  /// rusty_v8 note: Unlike in the C++ API, the isolate is entered when it is
  /// constructed and exited when dropped.
  #[inline(always)]
  pub unsafe fn enter(&self) {
    unsafe {
      v8__Isolate__Enter(self.as_real_ptr());
    }
  }

  /// Exits this isolate by restoring the previously entered one in the
  /// current thread.  The isolate may still stay the same, if it was
  /// entered more than once.
  ///
  /// Requires: self == Isolate::GetCurrent().
  ///
  /// rusty_v8 note: Unlike in the C++ API, the isolate is entered when it is
  /// constructed and exited when dropped.
  #[inline(always)]
  pub unsafe fn exit(&self) {
    unsafe {
      v8__Isolate__Exit(self.as_real_ptr());
    }
  }

  /// Disposes the isolate.  The isolate must not be entered by any
  /// thread to be disposable.
  pub(crate) unsafe fn dispose(&mut self) {
    /* No test case in rusty_v8 show this, but there have been situations in */
    /* deno where dropping Annex before the states causes a segfault. */
    unsafe {
      v8__Isolate__Dispose(self.as_real_ptr());
    }
  }

}
