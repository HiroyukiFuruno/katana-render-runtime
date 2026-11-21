use super::{ArrayBufferView, Data, Object, Opaque, TypedArray, Value};

/// An instance of BigInt64Array constructor.
#[repr(C)]
#[derive(Debug)]
pub struct BigInt64Array(Opaque);

impl_deref! { TypedArray for BigInt64Array }
impl_try_from! { Data for BigInt64Array if v => v.is_value() && cast::<Value>(v).is_big_int64_array() }
impl_try_from! { Value for BigInt64Array if v => v.is_big_int64_array() }
impl_try_from! { Object for BigInt64Array if v => v.is_big_int64_array() }
impl_try_from! { ArrayBufferView for BigInt64Array if v => v.is_big_int64_array() }
impl_try_from! { TypedArray for BigInt64Array if v => v.is_big_int64_array() }
impl_eq! { for BigInt64Array }
impl_hash! { for BigInt64Array use get_identity_hash }
impl_partial_eq! { Data for BigInt64Array use identity }
impl_partial_eq! { Value for BigInt64Array use identity }
impl_partial_eq! { Object for BigInt64Array use identity }
impl_partial_eq! { ArrayBufferView for BigInt64Array use identity }
impl_partial_eq! { TypedArray for BigInt64Array use identity }
impl_partial_eq! { BigInt64Array for BigInt64Array use identity }

/// An instance of BigUint64Array constructor.
#[repr(C)]
#[derive(Debug)]
pub struct BigUint64Array(Opaque);

impl_deref! { TypedArray for BigUint64Array }
impl_try_from! { Data for BigUint64Array if v => v.is_value() && cast::<Value>(v).is_big_uint64_array() }
impl_try_from! { Value for BigUint64Array if v => v.is_big_uint64_array() }
impl_try_from! { Object for BigUint64Array if v => v.is_big_uint64_array() }
impl_try_from! { ArrayBufferView for BigUint64Array if v => v.is_big_uint64_array() }
impl_try_from! { TypedArray for BigUint64Array if v => v.is_big_uint64_array() }
impl_eq! { for BigUint64Array }
impl_hash! { for BigUint64Array use get_identity_hash }
impl_partial_eq! { Data for BigUint64Array use identity }
impl_partial_eq! { Value for BigUint64Array use identity }
impl_partial_eq! { Object for BigUint64Array use identity }
impl_partial_eq! { ArrayBufferView for BigUint64Array use identity }
impl_partial_eq! { TypedArray for BigUint64Array use identity }
impl_partial_eq! { BigUint64Array for BigUint64Array use identity }

/// An instance of Float16Array constructor.
#[repr(C)]
#[derive(Debug)]
pub struct Float16Array(Opaque);

impl_deref! { TypedArray for Float16Array }
impl_try_from! { Data for Float16Array if v => v.is_value() && cast::<Value>(v).is_float16_array() }
impl_try_from! { Value for Float16Array if v => v.is_float16_array() }
impl_try_from! { Object for Float16Array if v => v.is_float16_array() }
impl_try_from! { ArrayBufferView for Float16Array if v => v.is_float16_array() }
impl_try_from! { TypedArray for Float16Array if v => v.is_float16_array() }
impl_eq! { for Float16Array }
impl_hash! { for Float16Array use get_identity_hash }
impl_partial_eq! { Data for Float16Array use identity }
impl_partial_eq! { Value for Float16Array use identity }
impl_partial_eq! { Object for Float16Array use identity }
impl_partial_eq! { ArrayBufferView for Float16Array use identity }
impl_partial_eq! { TypedArray for Float16Array use identity }
impl_partial_eq! { Float16Array for Float16Array use identity }

/// An instance of Float32Array constructor (ES6 draft 15.13.6).
#[repr(C)]
#[derive(Debug)]
pub struct Float32Array(Opaque);

impl_deref! { TypedArray for Float32Array }
impl_try_from! { Data for Float32Array if v => v.is_value() && cast::<Value>(v).is_float32_array() }
impl_try_from! { Value for Float32Array if v => v.is_float32_array() }
impl_try_from! { Object for Float32Array if v => v.is_float32_array() }
impl_try_from! { ArrayBufferView for Float32Array if v => v.is_float32_array() }
impl_try_from! { TypedArray for Float32Array if v => v.is_float32_array() }
impl_eq! { for Float32Array }
impl_hash! { for Float32Array use get_identity_hash }
impl_partial_eq! { Data for Float32Array use identity }
impl_partial_eq! { Value for Float32Array use identity }
impl_partial_eq! { Object for Float32Array use identity }
impl_partial_eq! { ArrayBufferView for Float32Array use identity }
impl_partial_eq! { TypedArray for Float32Array use identity }
impl_partial_eq! { Float32Array for Float32Array use identity }

/// An instance of Float64Array constructor (ES6 draft 15.13.6).
#[repr(C)]
#[derive(Debug)]
pub struct Float64Array(Opaque);

impl_deref! { TypedArray for Float64Array }
impl_try_from! { Data for Float64Array if v => v.is_value() && cast::<Value>(v).is_float64_array() }
impl_try_from! { Value for Float64Array if v => v.is_float64_array() }
impl_try_from! { Object for Float64Array if v => v.is_float64_array() }
impl_try_from! { ArrayBufferView for Float64Array if v => v.is_float64_array() }
impl_try_from! { TypedArray for Float64Array if v => v.is_float64_array() }
impl_eq! { for Float64Array }
impl_hash! { for Float64Array use get_identity_hash }
impl_partial_eq! { Data for Float64Array use identity }
impl_partial_eq! { Value for Float64Array use identity }
impl_partial_eq! { Object for Float64Array use identity }
impl_partial_eq! { ArrayBufferView for Float64Array use identity }
impl_partial_eq! { TypedArray for Float64Array use identity }
impl_partial_eq! { Float64Array for Float64Array use identity }
