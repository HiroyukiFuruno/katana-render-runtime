use super::{Data, Object, Opaque, Value};

/// A BigInt object (https://tc39.github.io/proposal-bigint)
#[repr(C)]
#[derive(Debug)]
pub struct BigIntObject(Opaque);

impl_deref! { Object for BigIntObject }
impl_try_from! { Data for BigIntObject if v => v.is_value() && cast::<Value>(v).is_big_int_object() }
impl_try_from! { Value for BigIntObject if v => v.is_big_int_object() }
impl_try_from! { Object for BigIntObject if v => v.is_big_int_object() }
impl_eq! { for BigIntObject }
impl_hash! { for BigIntObject use get_identity_hash }
impl_partial_eq! { Data for BigIntObject use identity }
impl_partial_eq! { Value for BigIntObject use identity }
impl_partial_eq! { Object for BigIntObject use identity }
impl_partial_eq! { BigIntObject for BigIntObject use identity }

/// A Boolean object (ECMA-262, 4.3.15).
#[repr(C)]
#[derive(Debug)]
pub struct BooleanObject(Opaque);

impl_deref! { Object for BooleanObject }
impl_try_from! { Data for BooleanObject if v => v.is_value() && cast::<Value>(v).is_boolean_object() }
impl_try_from! { Value for BooleanObject if v => v.is_boolean_object() }
impl_try_from! { Object for BooleanObject if v => v.is_boolean_object() }
impl_eq! { for BooleanObject }
impl_hash! { for BooleanObject use get_identity_hash }
impl_partial_eq! { Data for BooleanObject use identity }
impl_partial_eq! { Value for BooleanObject use identity }
impl_partial_eq! { Object for BooleanObject use identity }
impl_partial_eq! { BooleanObject for BooleanObject use identity }

/// A Number object (ECMA-262, 4.3.21).
#[repr(C)]
#[derive(Debug)]
pub struct NumberObject(Opaque);

impl_deref! { Object for NumberObject }
impl_try_from! { Data for NumberObject if v => v.is_value() && cast::<Value>(v).is_number_object() }
impl_try_from! { Value for NumberObject if v => v.is_number_object() }
impl_try_from! { Object for NumberObject if v => v.is_number_object() }
impl_eq! { for NumberObject }
impl_hash! { for NumberObject use get_identity_hash }
impl_partial_eq! { Data for NumberObject use identity }
impl_partial_eq! { Value for NumberObject use identity }
impl_partial_eq! { Object for NumberObject use identity }
impl_partial_eq! { NumberObject for NumberObject use identity }

/// A String object (ECMA-262, 4.3.18).
#[repr(C)]
#[derive(Debug)]
pub struct StringObject(Opaque);

impl_deref! { Object for StringObject }
impl_try_from! { Data for StringObject if v => v.is_value() && cast::<Value>(v).is_string_object() }
impl_try_from! { Value for StringObject if v => v.is_string_object() }
impl_try_from! { Object for StringObject if v => v.is_string_object() }
impl_eq! { for StringObject }
impl_hash! { for StringObject use get_identity_hash }
impl_partial_eq! { Data for StringObject use identity }
impl_partial_eq! { Value for StringObject use identity }
impl_partial_eq! { Object for StringObject use identity }
impl_partial_eq! { StringObject for StringObject use identity }

/// A Symbol object (ECMA-262 edition 6).
#[repr(C)]
#[derive(Debug)]
pub struct SymbolObject(Opaque);

impl_deref! { Object for SymbolObject }
impl_try_from! { Data for SymbolObject if v => v.is_value() && cast::<Value>(v).is_symbol_object() }
impl_try_from! { Value for SymbolObject if v => v.is_symbol_object() }
impl_try_from! { Object for SymbolObject if v => v.is_symbol_object() }
impl_eq! { for SymbolObject }
impl_hash! { for SymbolObject use get_identity_hash }
impl_partial_eq! { Data for SymbolObject use identity }
impl_partial_eq! { Value for SymbolObject use identity }
impl_partial_eq! { Object for SymbolObject use identity }
impl_partial_eq! { SymbolObject for SymbolObject use identity }
