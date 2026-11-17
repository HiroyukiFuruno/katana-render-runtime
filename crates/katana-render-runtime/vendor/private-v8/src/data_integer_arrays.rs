use super::{ArrayBufferView, Data, Object, Opaque, TypedArray, Value};

/// An instance of Int16Array constructor (ES6 draft 15.13.6).
#[repr(C)]
#[derive(Debug)]
pub struct Int16Array(Opaque);

impl_deref! { TypedArray for Int16Array }
impl_try_from! { Data for Int16Array if v => v.is_value() && cast::<Value>(v).is_int16_array() }
impl_try_from! { Value for Int16Array if v => v.is_int16_array() }
impl_try_from! { Object for Int16Array if v => v.is_int16_array() }
impl_try_from! { ArrayBufferView for Int16Array if v => v.is_int16_array() }
impl_try_from! { TypedArray for Int16Array if v => v.is_int16_array() }
impl_eq! { for Int16Array }
impl_hash! { for Int16Array use get_identity_hash }
impl_partial_eq! { Data for Int16Array use identity }
impl_partial_eq! { Value for Int16Array use identity }
impl_partial_eq! { Object for Int16Array use identity }
impl_partial_eq! { ArrayBufferView for Int16Array use identity }
impl_partial_eq! { TypedArray for Int16Array use identity }
impl_partial_eq! { Int16Array for Int16Array use identity }

/// An instance of Int32Array constructor (ES6 draft 15.13.6).
#[repr(C)]
#[derive(Debug)]
pub struct Int32Array(Opaque);

impl_deref! { TypedArray for Int32Array }
impl_try_from! { Data for Int32Array if v => v.is_value() && cast::<Value>(v).is_int32_array() }
impl_try_from! { Value for Int32Array if v => v.is_int32_array() }
impl_try_from! { Object for Int32Array if v => v.is_int32_array() }
impl_try_from! { ArrayBufferView for Int32Array if v => v.is_int32_array() }
impl_try_from! { TypedArray for Int32Array if v => v.is_int32_array() }
impl_eq! { for Int32Array }
impl_hash! { for Int32Array use get_identity_hash }
impl_partial_eq! { Data for Int32Array use identity }
impl_partial_eq! { Value for Int32Array use identity }
impl_partial_eq! { Object for Int32Array use identity }
impl_partial_eq! { ArrayBufferView for Int32Array use identity }
impl_partial_eq! { TypedArray for Int32Array use identity }
impl_partial_eq! { Int32Array for Int32Array use identity }

/// An instance of Int8Array constructor (ES6 draft 15.13.6).
#[repr(C)]
#[derive(Debug)]
pub struct Int8Array(Opaque);

impl_deref! { TypedArray for Int8Array }
impl_try_from! { Data for Int8Array if v => v.is_value() && cast::<Value>(v).is_int8_array() }
impl_try_from! { Value for Int8Array if v => v.is_int8_array() }
impl_try_from! { Object for Int8Array if v => v.is_int8_array() }
impl_try_from! { ArrayBufferView for Int8Array if v => v.is_int8_array() }
impl_try_from! { TypedArray for Int8Array if v => v.is_int8_array() }
impl_eq! { for Int8Array }
impl_hash! { for Int8Array use get_identity_hash }
impl_partial_eq! { Data for Int8Array use identity }
impl_partial_eq! { Value for Int8Array use identity }
impl_partial_eq! { Object for Int8Array use identity }
impl_partial_eq! { ArrayBufferView for Int8Array use identity }
impl_partial_eq! { TypedArray for Int8Array use identity }
impl_partial_eq! { Int8Array for Int8Array use identity }

/// An instance of Uint16Array constructor (ES6 draft 15.13.6).
#[repr(C)]
#[derive(Debug)]
pub struct Uint16Array(Opaque);

impl_deref! { TypedArray for Uint16Array }
impl_try_from! { Data for Uint16Array if v => v.is_value() && cast::<Value>(v).is_uint16_array() }
impl_try_from! { Value for Uint16Array if v => v.is_uint16_array() }
impl_try_from! { Object for Uint16Array if v => v.is_uint16_array() }
impl_try_from! { ArrayBufferView for Uint16Array if v => v.is_uint16_array() }
impl_try_from! { TypedArray for Uint16Array if v => v.is_uint16_array() }
impl_eq! { for Uint16Array }
impl_hash! { for Uint16Array use get_identity_hash }
impl_partial_eq! { Data for Uint16Array use identity }
impl_partial_eq! { Value for Uint16Array use identity }
impl_partial_eq! { Object for Uint16Array use identity }
impl_partial_eq! { ArrayBufferView for Uint16Array use identity }
impl_partial_eq! { TypedArray for Uint16Array use identity }
impl_partial_eq! { Uint16Array for Uint16Array use identity }

/// An instance of Uint32Array constructor (ES6 draft 15.13.6).
#[repr(C)]
#[derive(Debug)]
pub struct Uint32Array(Opaque);

impl_deref! { TypedArray for Uint32Array }
impl_try_from! { Data for Uint32Array if v => v.is_value() && cast::<Value>(v).is_uint32_array() }
impl_try_from! { Value for Uint32Array if v => v.is_uint32_array() }
impl_try_from! { Object for Uint32Array if v => v.is_uint32_array() }
impl_try_from! { ArrayBufferView for Uint32Array if v => v.is_uint32_array() }
impl_try_from! { TypedArray for Uint32Array if v => v.is_uint32_array() }
impl_eq! { for Uint32Array }
impl_hash! { for Uint32Array use get_identity_hash }
impl_partial_eq! { Data for Uint32Array use identity }
impl_partial_eq! { Value for Uint32Array use identity }
impl_partial_eq! { Object for Uint32Array use identity }
impl_partial_eq! { ArrayBufferView for Uint32Array use identity }
impl_partial_eq! { TypedArray for Uint32Array use identity }
impl_partial_eq! { Uint32Array for Uint32Array use identity }

/// An instance of Uint8Array constructor (ES6 draft 15.13.6).
#[repr(C)]
#[derive(Debug)]
pub struct Uint8Array(Opaque);

impl_deref! { TypedArray for Uint8Array }
impl_try_from! { Data for Uint8Array if v => v.is_value() && cast::<Value>(v).is_uint8_array() }
impl_try_from! { Value for Uint8Array if v => v.is_uint8_array() }
impl_try_from! { Object for Uint8Array if v => v.is_uint8_array() }
impl_try_from! { ArrayBufferView for Uint8Array if v => v.is_uint8_array() }
impl_try_from! { TypedArray for Uint8Array if v => v.is_uint8_array() }
impl_eq! { for Uint8Array }
impl_hash! { for Uint8Array use get_identity_hash }
impl_partial_eq! { Data for Uint8Array use identity }
impl_partial_eq! { Value for Uint8Array use identity }
impl_partial_eq! { Object for Uint8Array use identity }
impl_partial_eq! { ArrayBufferView for Uint8Array use identity }
impl_partial_eq! { TypedArray for Uint8Array use identity }
impl_partial_eq! { Uint8Array for Uint8Array use identity }

/// An instance of Uint8ClampedArray constructor (ES6 draft 15.13.6).
#[repr(C)]
#[derive(Debug)]
pub struct Uint8ClampedArray(Opaque);

impl_deref! { TypedArray for Uint8ClampedArray }
impl_try_from! { Data for Uint8ClampedArray if v => v.is_value() && cast::<Value>(v).is_uint8_clamped_array() }
impl_try_from! { Value for Uint8ClampedArray if v => v.is_uint8_clamped_array() }
impl_try_from! { Object for Uint8ClampedArray if v => v.is_uint8_clamped_array() }
impl_try_from! { ArrayBufferView for Uint8ClampedArray if v => v.is_uint8_clamped_array() }
impl_try_from! { TypedArray for Uint8ClampedArray if v => v.is_uint8_clamped_array() }
impl_eq! { for Uint8ClampedArray }
impl_hash! { for Uint8ClampedArray use get_identity_hash }
impl_partial_eq! { Data for Uint8ClampedArray use identity }
impl_partial_eq! { Value for Uint8ClampedArray use identity }
impl_partial_eq! { Object for Uint8ClampedArray use identity }
impl_partial_eq! { ArrayBufferView for Uint8ClampedArray use identity }
impl_partial_eq! { TypedArray for Uint8ClampedArray use identity }
impl_partial_eq! { Uint8ClampedArray for Uint8ClampedArray use identity }
