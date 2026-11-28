use crate::private_v8::Local;
use crate::private_v8::Private;
use crate::private_v8::String;
use crate::private_v8::isolate::RealIsolate;
use crate::private_v8::scope::PinScope;

unsafe extern "C" {
  fn v8__Private__ForApi(
    isolate: *mut RealIsolate,
    name: *const String,
  ) -> *const Private;
}

impl Private {
  /// Retrieve a global private symbol. If a symbol with this name has not
  /// been retrieved in the same isolate before, it is created.
  /// Note that private symbols created this way are never collected, so
  /// they should only be used for statically fixed properties.
  /// Also, there is only one global name space for the names used as keys.
  /// To minimize the potential for clashes, use qualified names as keys,
  /// e.g., "Class#property".
  #[inline(always)]
  pub fn for_api<'s>(
    scope: &PinScope<'s, '_, ()>,
    name: Option<Local<String>>,
  ) -> Local<'s, Private> {
    unsafe {
      scope.cast_local(|sd| {
        v8__Private__ForApi(
          sd.get_isolate_ptr(),
          name.map_or_else(std::ptr::null, |v| &*v),
        )
      })
    }
    .unwrap()
  }
}
