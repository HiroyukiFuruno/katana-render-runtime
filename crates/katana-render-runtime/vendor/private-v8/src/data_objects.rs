use super::{
  ArrayBufferView, BigInt64Array, BigIntObject, BigUint64Array, BooleanObject,
  Data, DataView, Date, Float32Array, Float64Array, Function, Int8Array,
  Int16Array, Int32Array, Map, NumberObject, Opaque, Promise,
  Proxy, RegExp, Set, SharedArrayBuffer, StringObject, SymbolObject,
  TypedArray, Uint8Array, Uint8ClampedArray, Uint16Array, Uint32Array, Value,
  WasmMemoryObject, WasmModuleObject,
};

/// A JavaScript object (ECMA-262, 4.3.3)
#[repr(C)]
#[derive(Debug)]
pub struct Object(Opaque);

impl_deref! { Value for Object }
impl_try_from! { Data for Object if v => v.is_value() && cast::<Value>(v).is_object() }
impl_try_from! { Value for Object if v => v.is_object() }
impl_from! { Array for Object }
impl_from! { ArrayBuffer for Object }
impl_from! { ArrayBufferView for Object }
impl_from! { DataView for Object }
impl_from! { TypedArray for Object }
impl_from! { BigInt64Array for Object }
impl_from! { BigUint64Array for Object }
impl_from! { Float32Array for Object }
impl_from! { Float64Array for Object }
impl_from! { Int16Array for Object }
impl_from! { Int32Array for Object }
impl_from! { Int8Array for Object }
impl_from! { Uint16Array for Object }
impl_from! { Uint32Array for Object }
impl_from! { Uint8Array for Object }
impl_from! { Uint8ClampedArray for Object }
impl_from! { BigIntObject for Object }
impl_from! { BooleanObject for Object }
impl_from! { Date for Object }
impl_from! { Function for Object }
impl_from! { Map for Object }
impl_from! { NumberObject for Object }
impl_from! { Promise for Object }
impl_from! { Proxy for Object }
impl_from! { RegExp for Object }
impl_from! { Set for Object }
impl_from! { SharedArrayBuffer for Object }
impl_from! { StringObject for Object }
impl_from! { SymbolObject for Object }
impl_from! { WasmMemoryObject for Object }
impl_from! { WasmModuleObject for Object }
impl_eq! { for Object }
impl_hash! { for Object use get_identity_hash }
impl_partial_eq! { Data for Object use identity }
impl_partial_eq! { Value for Object use identity }
impl_partial_eq! { Object for Object use identity }
impl_partial_eq! { Array for Object use identity }
impl_partial_eq! { ArrayBuffer for Object use identity }
impl_partial_eq! { ArrayBufferView for Object use identity }
impl_partial_eq! { DataView for Object use identity }
impl_partial_eq! { TypedArray for Object use identity }
impl_partial_eq! { BigInt64Array for Object use identity }
impl_partial_eq! { BigUint64Array for Object use identity }
impl_partial_eq! { Float32Array for Object use identity }
impl_partial_eq! { Float64Array for Object use identity }
impl_partial_eq! { Int16Array for Object use identity }
impl_partial_eq! { Int32Array for Object use identity }
impl_partial_eq! { Int8Array for Object use identity }
impl_partial_eq! { Uint16Array for Object use identity }
impl_partial_eq! { Uint32Array for Object use identity }
impl_partial_eq! { Uint8Array for Object use identity }
impl_partial_eq! { Uint8ClampedArray for Object use identity }
impl_partial_eq! { BigIntObject for Object use identity }
impl_partial_eq! { BooleanObject for Object use identity }
impl_partial_eq! { Date for Object use identity }
impl_partial_eq! { Function for Object use identity }
impl_partial_eq! { Map for Object use identity }
impl_partial_eq! { NumberObject for Object use identity }
impl_partial_eq! { Promise for Object use identity }
impl_partial_eq! { Proxy for Object use identity }
impl_partial_eq! { RegExp for Object use identity }
impl_partial_eq! { Set for Object use identity }
impl_partial_eq! { SharedArrayBuffer for Object use identity }
impl_partial_eq! { StringObject for Object use identity }
impl_partial_eq! { SymbolObject for Object use identity }
impl_partial_eq! { WasmMemoryObject for Object use identity }
impl_partial_eq! { WasmModuleObject for Object use identity }

/// An instance of the built-in array constructor (ECMA-262, 15.4.2).
#[repr(C)]
#[derive(Debug)]
pub struct Array(Opaque);

impl_deref! { Object for Array }
impl_try_from! { Data for Array if v => v.is_value() && cast::<Value>(v).is_array() }
impl_try_from! { Value for Array if v => v.is_array() }
impl_try_from! { Object for Array if v => v.is_array() }
impl_eq! { for Array }
impl_hash! { for Array use get_identity_hash }
impl_partial_eq! { Data for Array use identity }
impl_partial_eq! { Value for Array use identity }
impl_partial_eq! { Object for Array use identity }
impl_partial_eq! { Array for Array use identity }

/// An instance of the built-in ArrayBuffer constructor (ES6 draft 15.13.5).
#[repr(C)]
#[derive(Debug)]
pub struct ArrayBuffer(Opaque);

impl_deref! { Object for ArrayBuffer }
impl_try_from! { Data for ArrayBuffer if v => v.is_value() && cast::<Value>(v).is_array_buffer() }
impl_try_from! { Value for ArrayBuffer if v => v.is_array_buffer() }
impl_try_from! { Object for ArrayBuffer if v => v.is_array_buffer() }
impl_eq! { for ArrayBuffer }
impl_hash! { for ArrayBuffer use get_identity_hash }
impl_partial_eq! { Data for ArrayBuffer use identity }
impl_partial_eq! { Value for ArrayBuffer use identity }
impl_partial_eq! { Object for ArrayBuffer use identity }
impl_partial_eq! { ArrayBuffer for ArrayBuffer use identity }
