use super::*;
use crate::private_v8::CallbackScope;
use crate::private_v8::PinScope;

/* A trait object is a fat pointer, so box it once more before passing it through */
/* the FFI as a thin `*mut c_void`. */
struct InspectableData {
  imp: Box<dyn InspectableImpl>,
}

/// Supplies the value of an object added to the inspector's `$0` through `$4`
/// history.
pub trait InspectableImpl {
  /// Called by the inspector from within a V8 callback when the console
  /// dereferences one of `$0` through `$4`.
  ///
  /// There is no way to return an empty handle; if no value is available,
  /// return `undefined`.
  fn get<'s>(
    &self,
    scope: &mut PinScope<'s, '_>,
    context: Local<'s, Context>,
  ) -> Local<'s, Value>;
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorSession__Inspectable__BASE__get(
  rust_impl: *mut c_void,
  context: Local<Context>,
) -> *const Value {
  let data = unsafe { &*rust_impl.cast::<InspectableData>() };
  /* WHY: C++が開いたスコープで結果を確保し、C++側で呼び出し元に渡す。 */
  let scope = pin!(unsafe { CallbackScope::new(context) });
  let mut scope = scope.init();
  data.imp.get(&mut scope, context).as_non_null().as_ptr()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8_inspector__V8InspectorSession__Inspectable__BASE__DROP(
  rust_impl: *mut c_void,
) {
  unsafe {
    drop(Box::from_raw(rust_impl.cast::<InspectableData>()));
  }
}
