use super::{
  BigInt64Array, BigUint64Array, Data, Float32Array, Float64Array, Int8Array,
  Int16Array, Int32Array, Object, Opaque, Uint8Array, Uint8ClampedArray,
  Uint16Array, Uint32Array, Value,
};

/// A base class for an instance of one of "views" over ArrayBuffer,
/// including TypedArrays and DataView (ES6 draft 15.13).
#[repr(C)]
#[derive(Debug)]
pub struct ArrayBufferView(Opaque);

impl_deref! { Object for ArrayBufferView }
impl_try_from! { Data for ArrayBufferView if v => v.is_value() && cast::<Value>(v).is_array_buffer_view() }
impl_try_from! { Value for ArrayBufferView if v => v.is_array_buffer_view() }
impl_try_from! { Object for ArrayBufferView if v => v.is_array_buffer_view() }
impl_from! { DataView for ArrayBufferView }
impl_from! { TypedArray for ArrayBufferView }
impl_from! { BigInt64Array for ArrayBufferView }
impl_from! { BigUint64Array for ArrayBufferView }
impl_from! { Float32Array for ArrayBufferView }
impl_from! { Float64Array for ArrayBufferView }
impl_from! { Int16Array for ArrayBufferView }
impl_from! { Int32Array for ArrayBufferView }
impl_from! { Int8Array for ArrayBufferView }
impl_from! { Uint16Array for ArrayBufferView }
impl_from! { Uint32Array for ArrayBufferView }
impl_from! { Uint8Array for ArrayBufferView }
impl_from! { Uint8ClampedArray for ArrayBufferView }
impl_eq! { for ArrayBufferView }
impl_hash! { for ArrayBufferView use get_identity_hash }
impl_partial_eq! { Data for ArrayBufferView use identity }
impl_partial_eq! { Value for ArrayBufferView use identity }
impl_partial_eq! { Object for ArrayBufferView use identity }
impl_partial_eq! { ArrayBufferView for ArrayBufferView use identity }
impl_partial_eq! { DataView for ArrayBufferView use identity }
impl_partial_eq! { TypedArray for ArrayBufferView use identity }
impl_partial_eq! { BigInt64Array for ArrayBufferView use identity }
impl_partial_eq! { BigUint64Array for ArrayBufferView use identity }
impl_partial_eq! { Float32Array for ArrayBufferView use identity }
impl_partial_eq! { Float64Array for ArrayBufferView use identity }
impl_partial_eq! { Int16Array for ArrayBufferView use identity }
impl_partial_eq! { Int32Array for ArrayBufferView use identity }
impl_partial_eq! { Int8Array for ArrayBufferView use identity }
impl_partial_eq! { Uint16Array for ArrayBufferView use identity }
impl_partial_eq! { Uint32Array for ArrayBufferView use identity }
impl_partial_eq! { Uint8Array for ArrayBufferView use identity }
impl_partial_eq! { Uint8ClampedArray for ArrayBufferView use identity }

/// An instance of DataView constructor (ES6 draft 15.13.7).
#[repr(C)]
#[derive(Debug)]
pub struct DataView(Opaque);

impl_deref! { ArrayBufferView for DataView }
impl_try_from! { Data for DataView if v => v.is_value() && cast::<Value>(v).is_data_view() }
impl_try_from! { Value for DataView if v => v.is_data_view() }
impl_try_from! { Object for DataView if v => v.is_data_view() }
impl_try_from! { ArrayBufferView for DataView if v => v.is_data_view() }
impl_eq! { for DataView }
impl_hash! { for DataView use get_identity_hash }
impl_partial_eq! { Data for DataView use identity }
impl_partial_eq! { Value for DataView use identity }
impl_partial_eq! { Object for DataView use identity }
impl_partial_eq! { ArrayBufferView for DataView use identity }
impl_partial_eq! { DataView for DataView use identity }

/// A base class for an instance of TypedArray series of constructors
/// (ES6 draft 15.13.6).
#[repr(C)]
#[derive(Debug)]
pub struct TypedArray(Opaque);

impl_deref! { ArrayBufferView for TypedArray }
impl_try_from! { Data for TypedArray if v => v.is_value() && cast::<Value>(v).is_typed_array() }
impl_try_from! { Value for TypedArray if v => v.is_typed_array() }
impl_try_from! { Object for TypedArray if v => v.is_typed_array() }
impl_try_from! { ArrayBufferView for TypedArray if v => v.is_typed_array() }
impl_from! { BigInt64Array for TypedArray }
impl_from! { BigUint64Array for TypedArray }
impl_from! { Float32Array for TypedArray }
impl_from! { Float64Array for TypedArray }
impl_from! { Int16Array for TypedArray }
impl_from! { Int32Array for TypedArray }
impl_from! { Int8Array for TypedArray }
impl_from! { Uint16Array for TypedArray }
impl_from! { Uint32Array for TypedArray }
impl_from! { Uint8Array for TypedArray }
impl_from! { Uint8ClampedArray for TypedArray }
impl_eq! { for TypedArray }
impl_hash! { for TypedArray use get_identity_hash }
impl_partial_eq! { Data for TypedArray use identity }
impl_partial_eq! { Value for TypedArray use identity }
impl_partial_eq! { Object for TypedArray use identity }
impl_partial_eq! { ArrayBufferView for TypedArray use identity }
impl_partial_eq! { TypedArray for TypedArray use identity }
impl_partial_eq! { BigInt64Array for TypedArray use identity }
impl_partial_eq! { BigUint64Array for TypedArray use identity }
impl_partial_eq! { Float32Array for TypedArray use identity }
impl_partial_eq! { Float64Array for TypedArray use identity }
impl_partial_eq! { Int16Array for TypedArray use identity }
impl_partial_eq! { Int32Array for TypedArray use identity }
impl_partial_eq! { Int8Array for TypedArray use identity }
impl_partial_eq! { Uint16Array for TypedArray use identity }
impl_partial_eq! { Uint32Array for TypedArray use identity }
impl_partial_eq! { Uint8Array for TypedArray use identity }
impl_partial_eq! { Uint8ClampedArray for TypedArray use identity }
