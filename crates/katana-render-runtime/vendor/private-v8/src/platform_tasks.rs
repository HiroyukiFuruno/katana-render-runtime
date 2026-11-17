unsafe extern "C" {
  fn v8__Task__Run(task: *mut std::ffi::c_void);
  fn v8__Task__DELETE(task: *mut std::ffi::c_void);
  fn v8__IdleTask__Run(task: *mut std::ffi::c_void, deadline_in_seconds: f64);
  fn v8__IdleTask__DELETE(task: *mut std::ffi::c_void);
}

/// A V8 foreground task. Ownership is transferred from C++ to Rust when
/// V8 posts a task via [`super::PlatformImpl`] trait methods.
///
/// The embedder is responsible for scheduling the task and calling
/// [`run()`](Task::run). For example, in an async runtime like tokio:
///
/// ```ignore
/// tokio::spawn(async move { task.run() });
/// ```
///
/// If dropped without calling `run()`, the task is destroyed without
/// executing.
pub struct Task(pub(super) *mut std::ffi::c_void);

/* SAFETY: V8 tasks are designed to be posted from background threads and */
/* run on the isolate's foreground thread. The unique_ptr transfer is safe */
/* across thread boundaries. */
unsafe impl Send for Task {}

impl Task {
  /// Run the task. Consumes self to prevent double execution.
  pub fn run(self) {
    let ptr = self.0;
    /* Prevent Drop from deleting — we'll delete after Run. */
    std::mem::forget(self);
    unsafe {
      v8__Task__Run(ptr);
      v8__Task__DELETE(ptr);
    }
  }
}

impl Drop for Task {
  fn drop(&mut self) {
    unsafe { v8__Task__DELETE(self.0) };
  }
}

/// A V8 idle task. Similar to [`Task`] but accepts a deadline parameter
/// when run.
///
/// If dropped without calling `run()`, the task is destroyed without
/// executing.
pub struct IdleTask(pub(super) *mut std::ffi::c_void);

/* SAFETY: Same as Task — safe to transfer across threads. */
unsafe impl Send for IdleTask {}

impl IdleTask {
  /// Run the idle task with the given deadline. Consumes self.
  ///
  /// `deadline_in_seconds` is the absolute time (in seconds since some
  /// epoch) by which the idle task should complete.
  pub fn run(self, deadline_in_seconds: f64) {
    let ptr = self.0;
    std::mem::forget(self);
    unsafe {
      v8__IdleTask__Run(ptr, deadline_in_seconds);
      v8__IdleTask__DELETE(ptr);
    }
  }
}

impl Drop for IdleTask {
  fn drop(&mut self) {
    unsafe { v8__IdleTask__DELETE(self.0) };
  }
}
