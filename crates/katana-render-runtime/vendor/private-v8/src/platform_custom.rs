use super::{IdleTask, Task};

/// Trait for customizing platform behavior, following the same pattern as
/// `V8InspectorClientImpl`.
///
/// Implement this trait to receive V8 foreground tasks and schedule them
/// on your event loop. The C++ `CustomPlatform` wraps each isolate's
/// `TaskRunner` so that every `PostTask` / `PostDelayedTask` / etc. call
/// transfers task ownership to Rust through the corresponding trait method.
///
/// **The embedder is responsible for calling [`Task::run()`] on the
/// isolate's thread.** For example, using tokio:
///
/// ```ignore
/// fn post_task(&self, isolate_ptr: *mut c_void, task: Task) {
///     tokio::spawn(async move { task.run() });
/// }
///
/// fn post_delayed_task(&self, isolate_ptr: *mut c_void, task: Task, delay: f64) {
///     tokio::spawn(async move {
///         tokio::time::sleep(Duration::from_secs_f64(delay)).await;
///         task.run();
///     });
/// }
/// ```
///
/// All methods have default implementations that run the task immediately
/// (synchronously). Override to integrate with your event loop.
///
/// Implementations must be `Send + Sync` as callbacks may fire from any
/// thread.
#[allow(unused_variables)]
pub trait PlatformImpl: Send + Sync {
  /// Called when `TaskRunner::PostTask` is invoked for the given isolate.
  ///
  /// The [`Task`] must be run on the isolate's foreground thread by calling
  /// [`Task::run()`].
  ///
  /// May be called from ANY thread (V8 background threads, etc.).
  fn post_task(&self, isolate_ptr: *mut std::ffi::c_void, task: Task) {
    task.run();
  }

  /// Called when `TaskRunner::PostNonNestableTask` is invoked.
  ///
  /// Same semantics as [`post_task`](Self::post_task), but the task must
  /// not be run within a nested `PumpMessageLoop`.
  fn post_non_nestable_task(
    &self,
    isolate_ptr: *mut std::ffi::c_void,
    task: Task,
  ) {
    task.run();
  }

  /// Called when `TaskRunner::PostDelayedTask` is invoked.
  ///
  /// The task should be run after `delay_in_seconds` has elapsed.
  /// For example, using `tokio::time::sleep` or a timer wheel.
  ///
  /// May be called from ANY thread.
  fn post_delayed_task(
    &self,
    isolate_ptr: *mut std::ffi::c_void,
    task: Task,
    delay_in_seconds: f64,
  ) {
    task.run();
  }

  /// Called when `TaskRunner::PostNonNestableDelayedTask` is invoked.
  ///
  /// Same semantics as [`post_delayed_task`](Self::post_delayed_task).
  fn post_non_nestable_delayed_task(
    &self,
    isolate_ptr: *mut std::ffi::c_void,
    task: Task,
    delay_in_seconds: f64,
  ) {
    task.run();
  }

  /// Called when `TaskRunner::PostIdleTask` is invoked.
  ///
  /// The [`IdleTask`] should be run when the embedder has idle time,
  /// passing the deadline via [`IdleTask::run(deadline)`](IdleTask::run).
  fn post_idle_task(&self, isolate_ptr: *mut std::ffi::c_void, task: IdleTask) {
    task.run(0.0);
  }
}

/* FFI callbacks called from C++ CustomPlatform/CustomTaskRunner. */
/* `context` is a raw pointer to a `Box<dyn PlatformImpl>`. */
/* Task pointers are owned — Rust is responsible for running and deleting them. */

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__Platform__CustomPlatform__BASE__PostTask(
  context: *mut std::ffi::c_void,
  isolate: *mut std::ffi::c_void,
  task: *mut std::ffi::c_void,
) {
  let imp = unsafe { &*(context as *const Box<dyn PlatformImpl>) };
  imp.post_task(isolate, Task(task));
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__Platform__CustomPlatform__BASE__PostNonNestableTask(
  context: *mut std::ffi::c_void,
  isolate: *mut std::ffi::c_void,
  task: *mut std::ffi::c_void,
) {
  let imp = unsafe { &*(context as *const Box<dyn PlatformImpl>) };
  imp.post_non_nestable_task(isolate, Task(task));
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__Platform__CustomPlatform__BASE__PostDelayedTask(
  context: *mut std::ffi::c_void,
  isolate: *mut std::ffi::c_void,
  task: *mut std::ffi::c_void,
  delay_in_seconds: f64,
) {
  let imp = unsafe { &*(context as *const Box<dyn PlatformImpl>) };
  imp.post_delayed_task(isolate, Task(task), delay_in_seconds);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__Platform__CustomPlatform__BASE__PostNonNestableDelayedTask(
  context: *mut std::ffi::c_void,
  isolate: *mut std::ffi::c_void,
  task: *mut std::ffi::c_void,
  delay_in_seconds: f64,
) {
  let imp = unsafe { &*(context as *const Box<dyn PlatformImpl>) };
  imp.post_non_nestable_delayed_task(isolate, Task(task), delay_in_seconds);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__Platform__CustomPlatform__BASE__PostIdleTask(
  context: *mut std::ffi::c_void,
  isolate: *mut std::ffi::c_void,
  task: *mut std::ffi::c_void,
) {
  let imp = unsafe { &*(context as *const Box<dyn PlatformImpl>) };
  imp.post_idle_task(isolate, IdleTask(task));
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__Platform__CustomPlatform__BASE__DROP(
  context: *mut std::ffi::c_void,
) {
  unsafe {
    let _ = Box::from_raw(context as *mut Box<dyn PlatformImpl>);
  }
}
