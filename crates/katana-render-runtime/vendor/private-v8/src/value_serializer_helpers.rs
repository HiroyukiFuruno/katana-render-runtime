use super::*;

/// The ValueSerializerImpl trait allows for
/// custom callback functions used by v8.
pub trait ValueSerializerImpl {
  fn throw_data_clone_error<'s>(
    &self,
    scope: &mut PinScope<'s, '_>,
    message: Local<'s, String>,
  );

  fn has_custom_host_object(&self, _isolate: &Isolate) -> bool {
    false
  }

  fn is_host_object<'s>(
    &self,
    scope: &mut PinScope<'s, '_>,
    _object: Local<'s, Object>,
  ) -> Option<bool> {
    let msg =
      String::new(scope, "Deno serializer: is_host_object not implemented")
        .unwrap();
    let exc = Exception::error(scope, msg);
    scope.throw_exception(exc);
    None
  }

  fn write_host_object<'s>(
    &self,
    scope: &mut PinScope<'s, '_>,
    _object: Local<'s, Object>,
    _value_serializer: &CxxValueSerializer,
  ) -> Option<bool> {
    let msg =
      String::new(scope, "Deno serializer: write_host_object not implemented")
        .unwrap();
    let exc = Exception::error(scope, msg);
    scope.throw_exception(exc);
    None
  }

  fn get_shared_array_buffer_id<'s>(
    &self,
    _scope: &mut PinScope<'s, '_>,
    _shared_array_buffer: Local<'s, SharedArrayBuffer>,
  ) -> Option<u32> {
    None
  }

  fn get_wasm_module_transfer_id(
    &self,
    scope: &mut PinScope<'_, '_>,
    _module: Local<WasmModuleObject>,
  ) -> Option<u32> {
    let msg = String::new(
      scope,
      "Deno serializer: get_wasm_module_transfer_id not implemented",
    )
    .unwrap();
    let exc = Exception::error(scope, msg);
    scope.throw_exception(exc);
    None
  }
}
