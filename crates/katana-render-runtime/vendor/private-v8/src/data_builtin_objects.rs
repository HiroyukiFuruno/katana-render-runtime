use super::{Data, Object, Opaque, Value};

/// An instance of the built-in Date constructor (ECMA-262, 15.9).
#[repr(C)]
#[derive(Debug)]
pub struct Date(Opaque);

impl_deref! { Object for Date }
impl_try_from! { Data for Date if v => v.is_value() && cast::<Value>(v).is_date() }
impl_try_from! { Value for Date if v => v.is_date() }
impl_try_from! { Object for Date if v => v.is_date() }
impl_eq! { for Date }
impl_hash! { for Date use get_identity_hash }
impl_partial_eq! { Data for Date use identity }
impl_partial_eq! { Value for Date use identity }
impl_partial_eq! { Object for Date use identity }
impl_partial_eq! { Date for Date use identity }

/// A JavaScript function object (ECMA-262, 15.3).
#[repr(C)]
#[derive(Debug)]
pub struct Function(Opaque);

impl_deref! { Object for Function }
impl_try_from! { Data for Function if v => v.is_value() && cast::<Value>(v).is_function() }
impl_try_from! { Value for Function if v => v.is_function() }
impl_try_from! { Object for Function if v => v.is_function() }
impl_eq! { for Function }
impl_hash! { for Function use get_identity_hash }
impl_partial_eq! { Data for Function use identity }
impl_partial_eq! { Value for Function use identity }
impl_partial_eq! { Object for Function use identity }
impl_partial_eq! { Function for Function use identity }

/// An instance of the built-in Map constructor (ECMA-262, 6th Edition, 23.1.1).
#[repr(C)]
#[derive(Debug)]
pub struct Map(Opaque);

impl_deref! { Object for Map }
impl_try_from! { Data for Map if v => v.is_value() && cast::<Value>(v).is_map() }
impl_try_from! { Value for Map if v => v.is_map() }
impl_try_from! { Object for Map if v => v.is_map() }
impl_eq! { for Map }
impl_hash! { for Map use get_identity_hash }
impl_partial_eq! { Data for Map use identity }
impl_partial_eq! { Value for Map use identity }
impl_partial_eq! { Object for Map use identity }
impl_partial_eq! { Map for Map use identity }

/// An instance of the built-in Promise constructor (ES6 draft).
#[repr(C)]
#[derive(Debug)]
pub struct Promise(Opaque);

impl_deref! { Object for Promise }
impl_try_from! { Data for Promise if v => v.is_value() && cast::<Value>(v).is_promise() }
impl_try_from! { Value for Promise if v => v.is_promise() }
impl_try_from! { Object for Promise if v => v.is_promise() }
impl_eq! { for Promise }
impl_hash! { for Promise use get_identity_hash }
impl_partial_eq! { Data for Promise use identity }
impl_partial_eq! { Value for Promise use identity }
impl_partial_eq! { Object for Promise use identity }
impl_partial_eq! { Promise for Promise use identity }

/// An instance of the built-in Proxy constructor (ECMA-262, 6th Edition,
/// 26.2.1).
#[repr(C)]
#[derive(Debug)]
pub struct Proxy(Opaque);

impl_deref! { Object for Proxy }
impl_try_from! { Data for Proxy if v => v.is_value() && cast::<Value>(v).is_proxy() }
impl_try_from! { Value for Proxy if v => v.is_proxy() }
impl_try_from! { Object for Proxy if v => v.is_proxy() }
impl_eq! { for Proxy }
impl_hash! { for Proxy use get_identity_hash }
impl_partial_eq! { Data for Proxy use identity }
impl_partial_eq! { Value for Proxy use identity }
impl_partial_eq! { Object for Proxy use identity }
impl_partial_eq! { Proxy for Proxy use identity }

/// An instance of the built-in RegExp constructor (ECMA-262, 15.10).
#[repr(C)]
#[derive(Debug)]
pub struct RegExp(Opaque);

impl_deref! { Object for RegExp }
impl_try_from! { Data for RegExp if v => v.is_value() && cast::<Value>(v).is_reg_exp() }
impl_try_from! { Value for RegExp if v => v.is_reg_exp() }
impl_try_from! { Object for RegExp if v => v.is_reg_exp() }
impl_eq! { for RegExp }
impl_hash! { for RegExp use get_identity_hash }
impl_partial_eq! { Data for RegExp use identity }
impl_partial_eq! { Value for RegExp use identity }
impl_partial_eq! { Object for RegExp use identity }
impl_partial_eq! { RegExp for RegExp use identity }

/// An instance of the built-in Set constructor (ECMA-262, 6th Edition, 23.2.1).
#[repr(C)]
#[derive(Debug)]
pub struct Set(Opaque);

impl_deref! { Object for Set }
impl_try_from! { Data for Set if v => v.is_value() && cast::<Value>(v).is_set() }
impl_try_from! { Value for Set if v => v.is_set() }
impl_try_from! { Object for Set if v => v.is_set() }
impl_eq! { for Set }
impl_hash! { for Set use get_identity_hash }
impl_partial_eq! { Data for Set use identity }
impl_partial_eq! { Value for Set use identity }
impl_partial_eq! { Object for Set use identity }
impl_partial_eq! { Set for Set use identity }

/// An instance of the built-in SharedArrayBuffer constructor.
#[repr(C)]
#[derive(Debug)]
pub struct SharedArrayBuffer(Opaque);

impl_deref! { Object for SharedArrayBuffer }
impl_try_from! { Data for SharedArrayBuffer if v => v.is_value() && cast::<Value>(v).is_shared_array_buffer() }
impl_try_from! { Value for SharedArrayBuffer if v => v.is_shared_array_buffer() }
impl_try_from! { Object for SharedArrayBuffer if v => v.is_shared_array_buffer() }
impl_eq! { for SharedArrayBuffer }
impl_hash! { for SharedArrayBuffer use get_identity_hash }
impl_partial_eq! { Data for SharedArrayBuffer use identity }
impl_partial_eq! { Value for SharedArrayBuffer use identity }
impl_partial_eq! { Object for SharedArrayBuffer use identity }
impl_partial_eq! { SharedArrayBuffer for SharedArrayBuffer use identity }

#[repr(C)]
#[derive(Debug)]
pub struct WasmMemoryObject(Opaque);

impl_deref! { Object for WasmMemoryObject }
impl_try_from! { Data for WasmMemoryObject if v => v.is_value() && cast::<Value>(v).is_wasm_memory_object() }
impl_try_from! { Value for WasmMemoryObject if v => v.is_wasm_memory_object() }
impl_try_from! { Object for WasmMemoryObject if v => v.is_wasm_memory_object() }
impl_eq! { for WasmMemoryObject }
impl_hash! { for WasmMemoryObject use get_identity_hash }
impl_partial_eq! { Data for WasmMemoryObject use identity }
impl_partial_eq! { Value for WasmMemoryObject use identity }
impl_partial_eq! { Object for WasmMemoryObject use identity }
impl_partial_eq! { WasmMemoryObject for WasmMemoryObject use identity }

#[repr(C)]
#[derive(Debug)]
pub struct WasmModuleObject(Opaque);

impl_deref! { Object for WasmModuleObject }
impl_try_from! { Data for WasmModuleObject if v => v.is_value() && cast::<Value>(v).is_wasm_module_object() }
impl_try_from! { Value for WasmModuleObject if v => v.is_wasm_module_object() }
impl_try_from! { Object for WasmModuleObject if v => v.is_wasm_module_object() }
impl_eq! { for WasmModuleObject }
impl_hash! { for WasmModuleObject use get_identity_hash }
impl_partial_eq! { Data for WasmModuleObject use identity }
impl_partial_eq! { Value for WasmModuleObject use identity }
impl_partial_eq! { Object for WasmModuleObject use identity }
impl_partial_eq! { WasmModuleObject for WasmModuleObject use identity }
