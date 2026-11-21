use super::*;

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueSerializer__Delegate__ThrowDataCloneError(
  this: &CxxValueSerializerDelegate,
  message: Local<String>,
) {
  let value_serializer_heap = unsafe { ValueSerializerHeap::dispatch(this) };
  let mut isolate =
    unsafe { Isolate::from_raw_ptr(value_serializer_heap.isolate_ptr) };
  let scope = unsafe { CallbackScope::new(&mut isolate) };
  let scope = pin!(scope);
  let scope = &mut scope.init();
  let context = Local::new(scope, &value_serializer_heap.context);
  let mut scope = ContextScope::new(scope, context);
  value_serializer_heap
    .value_serializer_impl
    .throw_data_clone_error(&mut scope, message);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueSerializer__Delegate__HasCustomHostObject(
  this: &CxxValueSerializerDelegate,
  isolate: *mut RealIsolate,
) -> bool {
  let value_serializer_heap = unsafe { ValueSerializerHeap::dispatch(this) };
  let isolate = unsafe { Isolate::from_raw_ptr(isolate) };
  value_serializer_heap
    .value_serializer_impl
    .has_custom_host_object(&isolate)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueSerializer__Delegate__IsHostObject(
  this: &CxxValueSerializerDelegate,
  isolate: *mut RealIsolate,
  object: Local<Object>,
) -> MaybeBool {
  let value_serializer_heap = unsafe { ValueSerializerHeap::dispatch(this) };
  let mut isolate = unsafe { Isolate::from_raw_ptr(isolate) };
  let scope = unsafe { CallbackScope::new(&mut isolate) };
  let scope = pin!(scope);
  let scope = &mut scope.init();
  let context = Local::new(scope, &value_serializer_heap.context);
  let mut scope = ContextScope::new(scope, context);

  MaybeBool::from(
    value_serializer_heap
      .value_serializer_impl
      .is_host_object(&mut scope, object),
  )
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueSerializer__Delegate__WriteHostObject(
  this: &CxxValueSerializerDelegate,
  isolate: *mut RealIsolate,
  object: Local<Object>,
) -> MaybeBool {
  let value_serializer_heap = unsafe { ValueSerializerHeap::dispatch(this) };
  let mut isolate = unsafe { Isolate::from_raw_ptr(isolate) };
  let scope = unsafe { CallbackScope::new(&mut isolate) };
  let scope = pin!(scope);
  let scope = &mut scope.init();
  let context = Local::new(scope, &value_serializer_heap.context);
  let mut scope = ContextScope::new(scope, context);
  let value_serializer_impl =
    value_serializer_heap.value_serializer_impl.as_ref();
  MaybeBool::from(value_serializer_impl.write_host_object(
    &mut scope,
    object,
    &value_serializer_heap.cxx_value_serializer,
  ))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueSerializer__Delegate__GetSharedArrayBufferId(
  this: &CxxValueSerializerDelegate,
  isolate: *mut RealIsolate,
  shared_array_buffer: Local<SharedArrayBuffer>,
  clone_id: *mut u32,
) -> bool {
  let value_serializer_heap = unsafe { ValueSerializerHeap::dispatch(this) };
  let mut isolate = unsafe { Isolate::from_raw_ptr(isolate) };
  let scope = unsafe { CallbackScope::new(&mut isolate) };
  let scope = pin!(scope);
  let scope = &mut scope.init();
  let context = Local::new(scope, &value_serializer_heap.context);
  let mut scope = ContextScope::new(scope, context);
  match value_serializer_heap
    .value_serializer_impl
    .get_shared_array_buffer_id(&mut scope, shared_array_buffer)
  {
    Some(x) => {
      unsafe {
        *clone_id = x;
      }
      true
    }
    None => false,
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueSerializer__Delegate__GetWasmModuleTransferId(
  this: &CxxValueSerializerDelegate,
  isolate: *mut RealIsolate,
  module: Local<WasmModuleObject>,
  transfer_id: *mut u32,
) -> bool {
  let value_serializer_heap = unsafe { ValueSerializerHeap::dispatch(this) };
  let mut isolate = unsafe { Isolate::from_raw_ptr(isolate) };
  let scope = unsafe { CallbackScope::new(&mut isolate) };
  let scope = pin!(scope);
  let scope = &mut scope.init();
  let context = Local::new(scope, &value_serializer_heap.context);
  let scope = &mut ContextScope::new(scope, context);
  match value_serializer_heap
    .value_serializer_impl
    .get_wasm_module_transfer_id(scope, module)
  {
    Some(x) => {
      unsafe {
        *transfer_id = x;
      }
      true
    }
    None => false,
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueSerializer__Delegate__ReallocateBufferMemory(
  this: &CxxValueSerializerDelegate,
  old_buffer: *mut c_void,
  size: usize,
  actual_size: *mut usize,
) -> *mut c_void {
  let base = unsafe { ValueSerializerHeap::dispatch(this) };

  let buffer_size = base
    .buffer_size
    .swap(size, std::sync::atomic::Ordering::Release);
  let new_buffer = if old_buffer.is_null() {
    let layout = Layout::from_size_align(size, 1).unwrap();
    unsafe { alloc(layout) }
  } else {
    let old_layout = Layout::from_size_align(buffer_size, 1).unwrap();
    unsafe { realloc(old_buffer as *mut _, old_layout, size) }
  };

  unsafe {
    *actual_size = size;
  }
  new_buffer as *mut c_void
}

#[unsafe(no_mangle)]
unsafe extern "C" fn v8__ValueSerializer__Delegate__FreeBufferMemory(
  this: &mut CxxValueSerializerDelegate,
  buffer: *mut c_void,
) {
  let base = unsafe { ValueSerializerHeap::dispatch(this) };
  if !buffer.is_null() {
    let layout = Layout::from_size_align(
      base.buffer_size.load(std::sync::atomic::Ordering::Relaxed),
      1,
    )
    .unwrap();
    unsafe { dealloc(buffer as *mut _, layout) };
  };
}
