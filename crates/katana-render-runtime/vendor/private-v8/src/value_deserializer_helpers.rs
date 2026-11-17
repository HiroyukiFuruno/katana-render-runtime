use super::*;

/// The ValueDeserializerImpl trait allows for
/// custom callback functions used by v8.
pub trait ValueDeserializerImpl {
  fn read_host_object<'s>(
    &self,
    scope: &mut PinScope<'s, '_>,
    _value_deserializer: &CxxValueDeserializer,
  ) -> Option<Local<'s, Object>> {
    let msg =
      String::new(scope, "Deno deserializer: read_host_object not implemented")
        .unwrap();
    let exc = Exception::error(scope, msg);
    scope.throw_exception(exc);
    None
  }

  fn get_shared_array_buffer_from_id<'s>(
    &self,
    scope: &mut PinScope<'s, '_>,
    _transfer_id: u32,
  ) -> Option<Local<'s, SharedArrayBuffer>> {
    let msg = String::new(
      scope,
      "Deno deserializer: get_shared_array_buffer_from_id not implemented",
    )
    .unwrap();
    let exc = Exception::error(scope, msg);
    scope.throw_exception(exc);
    None
  }

  fn get_wasm_module_from_id<'s>(
    &self,
    scope: &mut PinScope<'s, '_>,
    _clone_id: u32,
  ) -> Option<Local<'s, WasmModuleObject>> {
    let msg = String::new(
      scope,
      "Deno deserializer: get_wasm_module_from_id not implemented",
    )
    .unwrap();
    let exc = Exception::error(scope, msg);
    scope.throw_exception(exc);
    None
  }
}
