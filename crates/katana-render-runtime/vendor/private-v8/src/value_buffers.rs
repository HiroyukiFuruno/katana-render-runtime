use crate::private_v8::Value;

unsafe extern "C" {
  fn v8__Value__IsArrayBuffer(this: *const Value) -> bool;
  fn v8__Value__IsArrayBufferView(this: *const Value) -> bool;
  fn v8__Value__IsTypedArray(this: *const Value) -> bool;
  fn v8__Value__IsUint8Array(this: *const Value) -> bool;
  fn v8__Value__IsUint8ClampedArray(this: *const Value) -> bool;
  fn v8__Value__IsInt8Array(this: *const Value) -> bool;
  fn v8__Value__IsUint16Array(this: *const Value) -> bool;
  fn v8__Value__IsInt16Array(this: *const Value) -> bool;
  fn v8__Value__IsUint32Array(this: *const Value) -> bool;
  fn v8__Value__IsInt32Array(this: *const Value) -> bool;
  fn v8__Value__IsFloat16Array(this: *const Value) -> bool;
  fn v8__Value__IsFloat32Array(this: *const Value) -> bool;
  fn v8__Value__IsFloat64Array(this: *const Value) -> bool;
  fn v8__Value__IsBigInt64Array(this: *const Value) -> bool;
  fn v8__Value__IsBigUint64Array(this: *const Value) -> bool;
  fn v8__Value__IsDataView(this: *const Value) -> bool;
  fn v8__Value__IsSharedArrayBuffer(this: *const Value) -> bool;
  fn v8__Value__IsProxy(this: *const Value) -> bool;
  fn v8__Value__IsWasmMemoryObject(this: *const Value) -> bool;
  fn v8__Value__IsWasmModuleObject(this: *const Value) -> bool;
}

impl Value {
  /// Returns true if this value is an ArrayBuffer.
  #[inline(always)]
  pub fn is_array_buffer(&self) -> bool {
    unsafe { v8__Value__IsArrayBuffer(self) }
  }

  /// Returns true if this value is an ArrayBufferView.
  #[inline(always)]
  pub fn is_array_buffer_view(&self) -> bool {
    unsafe { v8__Value__IsArrayBufferView(self) }
  }

  /// Returns true if this value is one of TypedArrays.
  #[inline(always)]
  pub fn is_typed_array(&self) -> bool {
    unsafe { v8__Value__IsTypedArray(self) }
  }

  /// Returns true if this value is an Uint8Array.
  #[inline(always)]
  pub fn is_uint8_array(&self) -> bool {
    unsafe { v8__Value__IsUint8Array(self) }
  }

  /// Returns true if this value is an Uint8ClampedArray.
  #[inline(always)]
  pub fn is_uint8_clamped_array(&self) -> bool {
    unsafe { v8__Value__IsUint8ClampedArray(self) }
  }

  /// Returns true if this value is an Int8Array.
  #[inline(always)]
  pub fn is_int8_array(&self) -> bool {
    unsafe { v8__Value__IsInt8Array(self) }
  }

  /// Returns true if this value is an Uint16Array.
  #[inline(always)]
  pub fn is_uint16_array(&self) -> bool {
    unsafe { v8__Value__IsUint16Array(self) }
  }

  /// Returns true if this value is an Int16Array.
  #[inline(always)]
  pub fn is_int16_array(&self) -> bool {
    unsafe { v8__Value__IsInt16Array(self) }
  }

  /// Returns true if this value is an Uint32Array.
  #[inline(always)]
  pub fn is_uint32_array(&self) -> bool {
    unsafe { v8__Value__IsUint32Array(self) }
  }

  /// Returns true if this value is an Int32Array.
  #[inline(always)]
  pub fn is_int32_array(&self) -> bool {
    unsafe { v8__Value__IsInt32Array(self) }
  }

  /// Returns true if this value is a Float16Array.
  #[inline(always)]
  pub fn is_float16_array(&self) -> bool {
    unsafe { v8__Value__IsFloat16Array(self) }
  }

  /// Returns true if this value is a Float32Array.
  #[inline(always)]
  pub fn is_float32_array(&self) -> bool {
    unsafe { v8__Value__IsFloat32Array(self) }
  }

  /// Returns true if this value is a Float64Array.
  #[inline(always)]
  pub fn is_float64_array(&self) -> bool {
    unsafe { v8__Value__IsFloat64Array(self) }
  }

  /// Returns true if this value is a BigInt64Array.
  #[inline(always)]
  pub fn is_big_int64_array(&self) -> bool {
    unsafe { v8__Value__IsBigInt64Array(self) }
  }

  /// Returns true if this value is a BigUint64Array.
  #[inline(always)]
  pub fn is_big_uint64_array(&self) -> bool {
    unsafe { v8__Value__IsBigUint64Array(self) }
  }

  /// Returns true if this value is a DataView.
  #[inline(always)]
  pub fn is_data_view(&self) -> bool {
    unsafe { v8__Value__IsDataView(self) }
  }

  /// Returns true if this value is a SharedArrayBuffer.
  /// This is an experimental feature.
  #[inline(always)]
  pub fn is_shared_array_buffer(&self) -> bool {
    unsafe { v8__Value__IsSharedArrayBuffer(self) }
  }

  /// Returns true if this value is a JavaScript Proxy.
  #[inline(always)]
  pub fn is_proxy(&self) -> bool {
    unsafe { v8__Value__IsProxy(self) }
  }

  /// Returns true if this value is a WasmMemoryObject.
  #[inline(always)]
  pub fn is_wasm_memory_object(&self) -> bool {
    unsafe { v8__Value__IsWasmMemoryObject(self) }
  }

  /// Returns true if this value is a WasmModuleObject.
  #[inline(always)]
  pub fn is_wasm_module_object(&self) -> bool {
    unsafe { v8__Value__IsWasmModuleObject(self) }
  }

}
