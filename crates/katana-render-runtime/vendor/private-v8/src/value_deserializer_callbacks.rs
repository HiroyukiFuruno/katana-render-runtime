use super::*;

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueDeserializer__Delegate__ReadHostObject(
  this: &CxxValueDeserializerDelegate,
  isolate: *mut RealIsolate,
) -> *const Object {
  let value_deserializer_heap =
    unsafe { ValueDeserializerHeap::dispatch(this) };
  let mut isolate = unsafe { Isolate::from_raw_ptr(isolate) };
  let scope = unsafe { CallbackScope::new(&mut isolate) };
  let scope = pin!(scope);
  let scope = &mut scope.init();
  let context = Local::new(scope, &value_deserializer_heap.context);
  let mut scope = { ContextScope::new(scope, context) };

  match value_deserializer_heap
    .value_deserializer_impl
    .read_host_object(
      &mut scope,
      &value_deserializer_heap.cxx_value_deserializer,
    ) {
    None => std::ptr::null(),
    Some(x) => x.as_non_null().as_ptr(),
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueDeserializer__Delegate__GetSharedArrayBufferFromId(
  this: &CxxValueDeserializerDelegate,
  isolate: *mut RealIsolate,
  transfer_id: u32,
) -> *const SharedArrayBuffer {
  let value_deserializer_heap =
    unsafe { ValueDeserializerHeap::dispatch(this) };
  let mut isolate = unsafe { Isolate::from_raw_ptr(isolate) };
  let scope = unsafe { CallbackScope::new(&mut isolate) };
  let scope = pin!(scope);
  let scope = &mut scope.init();
  /* let hs = scope; */
  let context = Local::new(scope, &value_deserializer_heap.context);
  let mut scope = { ContextScope::new(scope, context) };

  match value_deserializer_heap
    .value_deserializer_impl
    .get_shared_array_buffer_from_id(&mut scope, transfer_id)
  {
    None => std::ptr::null(),
    Some(x) => x.as_non_null().as_ptr(),
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueDeserializer__Delegate__GetWasmModuleFromId(
  this: &mut CxxValueDeserializerDelegate,
  isolate: *mut RealIsolate,
  clone_id: u32,
) -> *const WasmModuleObject {
  let value_deserializer_heap =
    unsafe { ValueDeserializerHeap::dispatch(this) };
  let mut isolate = unsafe { Isolate::from_raw_ptr(isolate) };
  let scope = unsafe { CallbackScope::new(&mut isolate) };
  let scope = pin!(scope);
  let scope = &mut scope.init();
  let context = Local::new(scope, &value_deserializer_heap.context);
  let mut scope = { ContextScope::new(scope, context) };

  match value_deserializer_heap
    .value_deserializer_impl
    .get_wasm_module_from_id(&mut scope, clone_id)
  {
    None => std::ptr::null(),
    Some(x) => x.as_non_null().as_ptr(),
  }
}
