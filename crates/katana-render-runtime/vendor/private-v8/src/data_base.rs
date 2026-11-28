use super::{
  AccessorSignature, Array, ArrayBuffer, ArrayBufferView, BigInt,
  BigInt64Array, BigIntObject, BigUint64Array, Boolean, BooleanObject, Context,
  DataView, Date, External, FixedArray, Float32Array, Float64Array, Function,
  FunctionTemplate, Int8Array, Int16Array, Int32, Int32Array, Integer, Map,
  Module, ModuleRequest, Name, Number, NumberObject, Object, ObjectTemplate,
  Opaque, Primitive, Private, Promise, Proxy,
  RegExp, Set, SharedArrayBuffer, String, StringObject, Symbol,
  SymbolObject, Template, TypedArray, Uint8Array, Uint8ClampedArray,
  Uint16Array, Uint32, Uint32Array, UnboundModuleScript, UnboundScript, Value,
  WasmMemoryObject, WasmModuleObject,
};

/// The superclass of objects that can reside on V8's heap.
#[repr(C)]
#[derive(Debug)]
pub struct Data(Opaque);

impl_from! { AccessorSignature for Data }
impl_from! { Context for Data }
impl_from! { FixedArray for Data }
impl_from! { Module for Data }
impl_from! { ModuleRequest for Data }
impl_from! { Private for Data }
impl_from! { Template for Data }
impl_from! { FunctionTemplate for Data }
impl_from! { ObjectTemplate for Data }
impl_from! { UnboundModuleScript for Data }
impl_from! { UnboundScript for Data }
impl_from! { Value for Data }
impl_from! { External for Data }
impl_from! { Object for Data }
impl_from! { Array for Data }
impl_from! { ArrayBuffer for Data }
impl_from! { ArrayBufferView for Data }
impl_from! { DataView for Data }
impl_from! { TypedArray for Data }
impl_from! { BigInt64Array for Data }
impl_from! { BigUint64Array for Data }
impl_from! { Float32Array for Data }
impl_from! { Float64Array for Data }
impl_from! { Int16Array for Data }
impl_from! { Int32Array for Data }
impl_from! { Int8Array for Data }
impl_from! { Uint16Array for Data }
impl_from! { Uint32Array for Data }
impl_from! { Uint8Array for Data }
impl_from! { Uint8ClampedArray for Data }
impl_from! { BigIntObject for Data }
impl_from! { BooleanObject for Data }
impl_from! { Date for Data }
impl_from! { Function for Data }
impl_from! { Map for Data }
impl_from! { NumberObject for Data }
impl_from! { Promise for Data }
impl_from! { Proxy for Data }
impl_from! { RegExp for Data }
impl_from! { Set for Data }
impl_from! { SharedArrayBuffer for Data }
impl_from! { StringObject for Data }
impl_from! { SymbolObject for Data }
impl_from! { WasmMemoryObject for Data }
impl_from! { WasmModuleObject for Data }
impl_from! { Primitive for Data }
impl_from! { BigInt for Data }
impl_from! { Boolean for Data }
impl_from! { Name for Data }
impl_from! { String for Data }
impl_from! { Symbol for Data }
impl_from! { Number for Data }
impl_from! { Integer for Data }
impl_from! { Int32 for Data }
impl_from! { Uint32 for Data }
impl_partial_eq! { AccessorSignature for Data use identity }
impl_partial_eq! { Context for Data use identity }
impl_partial_eq! { FixedArray for Data use identity }
impl_partial_eq! { Module for Data use identity }
impl_partial_eq! { ModuleRequest for Data use identity }
impl_partial_eq! { Private for Data use identity }
impl_partial_eq! { Template for Data use identity }
impl_partial_eq! { FunctionTemplate for Data use identity }
impl_partial_eq! { ObjectTemplate for Data use identity }
impl_partial_eq! { UnboundModuleScript for Data use identity }
impl_partial_eq! { External for Data use identity }
impl_partial_eq! { Object for Data use identity }
impl_partial_eq! { Array for Data use identity }
impl_partial_eq! { ArrayBuffer for Data use identity }
impl_partial_eq! { ArrayBufferView for Data use identity }
impl_partial_eq! { DataView for Data use identity }
impl_partial_eq! { TypedArray for Data use identity }
impl_partial_eq! { BigInt64Array for Data use identity }
impl_partial_eq! { BigUint64Array for Data use identity }
impl_partial_eq! { Float32Array for Data use identity }
impl_partial_eq! { Float64Array for Data use identity }
impl_partial_eq! { Int16Array for Data use identity }
impl_partial_eq! { Int32Array for Data use identity }
impl_partial_eq! { Int8Array for Data use identity }
impl_partial_eq! { Uint16Array for Data use identity }
impl_partial_eq! { Uint32Array for Data use identity }
impl_partial_eq! { Uint8Array for Data use identity }
impl_partial_eq! { Uint8ClampedArray for Data use identity }
impl_partial_eq! { BigIntObject for Data use identity }
impl_partial_eq! { BooleanObject for Data use identity }
impl_partial_eq! { Date for Data use identity }
impl_partial_eq! { Function for Data use identity }
impl_partial_eq! { Map for Data use identity }
impl_partial_eq! { NumberObject for Data use identity }
impl_partial_eq! { Promise for Data use identity }
impl_partial_eq! { Proxy for Data use identity }
impl_partial_eq! { RegExp for Data use identity }
impl_partial_eq! { Set for Data use identity }
impl_partial_eq! { SharedArrayBuffer for Data use identity }
impl_partial_eq! { StringObject for Data use identity }
impl_partial_eq! { SymbolObject for Data use identity }
impl_partial_eq! { WasmMemoryObject for Data use identity }
impl_partial_eq! { WasmModuleObject for Data use identity }
impl_partial_eq! { Boolean for Data use identity }
impl_partial_eq! { Symbol for Data use identity }
