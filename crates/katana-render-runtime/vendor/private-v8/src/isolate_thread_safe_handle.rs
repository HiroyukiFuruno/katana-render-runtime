use super::{Isolate, RealIsolate};
use std::cell::UnsafeCell;
use std::fmt::{self, Formatter};
use std::ptr::{NonNull, null_mut};
use std::sync::{Arc, Mutex};

unsafe extern "C" {
  pub(super) fn v8__Isolate__TerminateExecution(isolate: *const RealIsolate);
}

struct IsolateHandleInner {
  /// Safety invariants:
  /// - The 'main thread' must lock the mutex and reset `isolate` to null just
  ///   before the isolate is disposed.
  /// - Any other thread must lock the mutex while it's reading/using the
  ///   `isolate` pointer.
  /* These two fields can be replaced with a `Mutex<*mut RealIsolate>` once */
  /* `Mutex::data_ptr()` is stabilized. */
  isolate: UnsafeCell<*mut RealIsolate>,
  isolate_mutex: Mutex<()>,
}

unsafe impl Send for IsolateHandleInner {}
unsafe impl Sync for IsolateHandleInner {}

/// IsolateHandle is a thread-safe reference to an Isolate. Its main use is to
/// terminate execution of a running isolate from another thread.
///
/// It is created with [`Isolate::thread_safe_handle()`].
///
/// IsolateHandle is Cloneable, Send, and Sync.
#[derive(Clone)]
pub struct IsolateHandle(Arc<IsolateHandleInner>);

impl fmt::Debug for IsolateHandle {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let Ok(_lock) = self.0.isolate_mutex.try_lock() else {
      return f.debug_struct("IsolateHandle").finish_non_exhaustive();
    };
    /* SAFETY: mutex lock is held */
    let ptr = unsafe { *self.0.isolate.get() };
    f.debug_struct("IsolateHandle")
      .field("isolate_ptr", &ptr)
      .finish()
  }
}

impl IsolateHandle {
  #[inline(always)]
  pub(super) fn new(isolate: &Isolate) -> Self {
    let inner = Arc::new(IsolateHandleInner {
      isolate: UnsafeCell::new(isolate.as_real_ptr()),
      isolate_mutex: Mutex::new(()),
    });
    Self(inner)
  }

  /// Set the inner isolate pointer to null.
  pub(super) fn dispose(&self) {
    let _lock = self.0.isolate_mutex.lock().unwrap();
    /* SAFETY: mutex lock is held */
    unsafe { *self.0.isolate.get() = null_mut() }
  }

  /// Access the isolate, if it hasn't yet been disposed of.
  ///
  /// A lock is taken on the pointer and held for the scope of `f`, which
  /// means the isolate can't be dropped until after `f` returns. If you
  /// do something with the isolate afterwards, that needs to be verified
  /// to be safe separately.
  pub(crate) fn with_isolate_ptr<R>(
    &self,
    f: impl FnOnce(NonNull<RealIsolate>) -> R,
  ) -> Option<R> {
    let _lock = self.0.isolate_mutex.lock().unwrap();
    /* SAFETY: mutex lock is held */
    let ptr = unsafe { *self.0.isolate.get() };
    NonNull::new(ptr).map(f)
  }

  /// Access the pointer for this isolate - it may be null.
  ///
  /// # Safety
  /// This function must only be called from the main thread associated with
  /// the V8 isolate.
  /* TODO: have this return an `Option<NonNull<RealIsolate>>` */
  pub(crate) unsafe fn get_isolate_ptr(&self) -> *mut RealIsolate {
    /* SAFETY: this function must only be called from the main thread of the */
    /* isolate. On that thread, the caller cannot race with teardown code that */
    /* sets this pointer to null. */
    unsafe { *self.0.isolate.get() }
  }

  /// Forcefully terminate the current thread of JavaScript execution
  /// in the given isolate.
  ///
  /// This method can be used by any thread even if that thread has not
  /// acquired the V8 lock with a Locker object.
  ///
  /// Returns false if Isolate was already destroyed.
  #[inline(always)]
  pub fn terminate_execution(&self) -> bool {
    self
      .with_isolate_ptr(|isolate| unsafe {
        v8__Isolate__TerminateExecution(isolate.as_ptr())
      })
      .is_some()
  }



}
