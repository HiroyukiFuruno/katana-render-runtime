use super::{
  Array, ArrayBuffer, ArrayBufferView, BigInt, BigInt64Array, BigIntObject,
  BigUint64Array, Boolean, BooleanObject, Data, DataView, Date, Float32Array,
  Float64Array, Function, Int8Array, Int16Array, Int32, Int32Array, Integer,
  Map, Name, Number, NumberObject, Object, Opaque, Primitive, Promise,
  Proxy, RegExp, Set, SharedArrayBuffer, String, StringObject,
  Symbol, SymbolObject, TypedArray, Uint8Array, Uint8ClampedArray, Uint16Array,
  Uint32, Uint32Array, WasmMemoryObject, WasmModuleObject,
};

/// The superclass of all JavaScript values and objects.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Value(Opaque);

impl_deref! { Data for Value }
impl_try_from! { Data for Value if v => v.is_value() }
impl_from! { External for Value }
impl_from! { Object for Value }
impl_from! { Array for Value }
impl_from! { ArrayBuffer for Value }
impl_from! { ArrayBufferView for Value }
impl_from! { DataView for Value }
impl_from! { TypedArray for Value }
impl_from! { BigInt64Array for Value }
impl_from! { BigUint64Array for Value }
impl_from! { Float32Array for Value }
impl_from! { Float64Array for Value }
impl_from! { Int16Array for Value }
impl_from! { Int32Array for Value }
impl_from! { Int8Array for Value }
impl_from! { Uint16Array for Value }
impl_from! { Uint32Array for Value }
impl_from! { Uint8Array for Value }
impl_from! { Uint8ClampedArray for Value }
impl_from! { BigIntObject for Value }
impl_from! { BooleanObject for Value }
impl_from! { Date for Value }
impl_from! { Function for Value }
impl_from! { Map for Value }
impl_from! { NumberObject for Value }
impl_from! { Promise for Value }
impl_from! { Proxy for Value }
impl_from! { RegExp for Value }
impl_from! { Set for Value }
impl_from! { SharedArrayBuffer for Value }
impl_from! { StringObject for Value }
impl_from! { SymbolObject for Value }
impl_from! { WasmMemoryObject for Value }
impl_from! { WasmModuleObject for Value }
impl_from! { Primitive for Value }
impl_from! { BigInt for Value }
impl_from! { Boolean for Value }
impl_from! { Name for Value }
impl_from! { String for Value }
impl_from! { Symbol for Value }
impl_from! { Number for Value }
impl_from! { Integer for Value }
impl_from! { Int32 for Value }
impl_from! { Uint32 for Value }
impl_eq! { for Value }
impl_hash! { for Value use get_hash }
impl_partial_eq! { Value for Value use same_value_zero }
impl_partial_eq! { External for Value use identity }
impl_partial_eq! { Object for Value use identity }
impl_partial_eq! { Array for Value use identity }
impl_partial_eq! { ArrayBuffer for Value use identity }
impl_partial_eq! { ArrayBufferView for Value use identity }
impl_partial_eq! { DataView for Value use identity }
impl_partial_eq! { TypedArray for Value use identity }
impl_partial_eq! { BigInt64Array for Value use identity }
impl_partial_eq! { BigUint64Array for Value use identity }
impl_partial_eq! { Float32Array for Value use identity }
impl_partial_eq! { Float64Array for Value use identity }
impl_partial_eq! { Int16Array for Value use identity }
impl_partial_eq! { Int32Array for Value use identity }
impl_partial_eq! { Int8Array for Value use identity }
impl_partial_eq! { Uint16Array for Value use identity }
impl_partial_eq! { Uint32Array for Value use identity }
impl_partial_eq! { Uint8Array for Value use identity }
impl_partial_eq! { Uint8ClampedArray for Value use identity }
impl_partial_eq! { BigIntObject for Value use identity }
impl_partial_eq! { BooleanObject for Value use identity }
impl_partial_eq! { Date for Value use identity }
impl_partial_eq! { Function for Value use identity }
impl_partial_eq! { Map for Value use identity }
impl_partial_eq! { NumberObject for Value use identity }
impl_partial_eq! { Promise for Value use identity }
impl_partial_eq! { Proxy for Value use identity }
impl_partial_eq! { RegExp for Value use identity }
impl_partial_eq! { Set for Value use identity }
impl_partial_eq! { SharedArrayBuffer for Value use identity }
impl_partial_eq! { StringObject for Value use identity }
impl_partial_eq! { SymbolObject for Value use identity }
impl_partial_eq! { WasmMemoryObject for Value use identity }
impl_partial_eq! { WasmModuleObject for Value use identity }
impl_partial_eq! { Primitive for Value use same_value_zero }
impl_partial_eq! { BigInt for Value use same_value_zero }
impl_partial_eq! { Boolean for Value use identity }
impl_partial_eq! { Name for Value use same_value_zero }
impl_partial_eq! { String for Value use same_value_zero }
impl_partial_eq! { Symbol for Value use identity }
impl_partial_eq! { Number for Value use same_value_zero }
impl_partial_eq! { Integer for Value use same_value_zero }
impl_partial_eq! { Int32 for Value use same_value_zero }
impl_partial_eq! { Uint32 for Value use same_value_zero }

/// A JavaScript value that wraps a C++ void*. This type of value is mainly used
/// to associate C++ data structures with JavaScript objects.
#[repr(C)]
#[derive(Debug)]
pub struct External(Opaque);

impl_deref! { Value for External }
impl_try_from! { Data for External if v => v.is_value() && cast::<Value>(v).is_external() }
impl_try_from! { Value for External if v => v.is_external() }
impl_eq! { for External }
impl_hash! { for External use get_hash }
impl_partial_eq! { Data for External use identity }
impl_partial_eq! { Value for External use identity }
impl_partial_eq! { External for External use identity }
