use super::Platform;
use crate::private_v8::Isolate;
use crate::private_v8::isolate::RealIsolate;
use crate::private_v8::support::SharedRef;

unsafe extern "C" {


  fn v8__Platform__NotifyIsolateShutdown(
    platform: *mut Platform,
    isolate: *mut RealIsolate,
  );
}

impl Platform {


  /// Notifies the given platform about the Isolate getting deleted soon. Has to
  /// be called for all Isolates which are deleted - unless we're shutting down
  /// the platform.
  ///
  /// The |platform| has to be created using |NewDefaultPlatform|.
  #[inline(always)]
  pub(crate) unsafe fn notify_isolate_shutdown(
    platform: &SharedRef<Self>,
    isolate: &Isolate,
  ) {
    unsafe {
      v8__Platform__NotifyIsolateShutdown(
        &**platform as *const Self as *mut _,
        isolate.as_real_ptr(),
      );
    }
  }
}
