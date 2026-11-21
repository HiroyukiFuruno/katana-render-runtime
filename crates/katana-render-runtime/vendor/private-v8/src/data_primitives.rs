use super::{Data, Int32, Integer, Number, Opaque, Uint32, Value};

/// The superclass of primitive values. See ECMA-262 4.3.2.
#[repr(C)]
#[derive(Debug)]
pub struct Primitive(Opaque);

impl_deref! { Value for Primitive }
impl_try_from! { Data for Primitive if v => v.is_primitive() }
impl_try_from! { Value for Primitive if v => v.is_primitive() }
impl_from! { BigInt for Primitive }
impl_from! { Boolean for Primitive }
impl_from! { Name for Primitive }
impl_from! { String for Primitive }
impl_from! { Symbol for Primitive }
impl_from! { Number for Primitive }
impl_from! { Integer for Primitive }
impl_from! { Int32 for Primitive }
impl_from! { Uint32 for Primitive }
impl_eq! { for Primitive }
impl_hash! { for Primitive use get_hash }
impl_partial_eq! { Value for Primitive use same_value_zero }
impl_partial_eq! { Primitive for Primitive use same_value_zero }
impl_partial_eq! { BigInt for Primitive use same_value_zero }
impl_partial_eq! { Boolean for Primitive use identity }
impl_partial_eq! { Name for Primitive use same_value_zero }
impl_partial_eq! { String for Primitive use same_value_zero }
impl_partial_eq! { Symbol for Primitive use identity }
impl_partial_eq! { Number for Primitive use same_value_zero }
impl_partial_eq! { Integer for Primitive use same_value_zero }
impl_partial_eq! { Int32 for Primitive use same_value_zero }
impl_partial_eq! { Uint32 for Primitive use same_value_zero }

/// A JavaScript BigInt value (https://tc39.github.io/proposal-bigint)
#[repr(C)]
#[derive(Debug)]
pub struct BigInt(Opaque);

impl_deref! { Primitive for BigInt }
impl_try_from! { Data for BigInt if v => v.is_big_int() }
impl_try_from! { Value for BigInt if v => v.is_big_int() }
impl_try_from! { Primitive for BigInt if v => v.is_big_int() }
impl_eq! { for BigInt }
impl_hash! { for BigInt use get_hash }
impl_partial_eq! { Value for BigInt use same_value_zero }
impl_partial_eq! { Primitive for BigInt use same_value_zero }
impl_partial_eq! { BigInt for BigInt use strict_equals }

/// A primitive boolean value (ECMA-262, 4.3.14). Either the true
/// or false value.
#[repr(C)]
#[derive(Debug)]
pub struct Boolean(Opaque);

impl_deref! { Primitive for Boolean }
impl_try_from! { Data for Boolean if v => v.is_boolean() }
impl_try_from! { Value for Boolean if v => v.is_boolean() }
impl_try_from! { Primitive for Boolean if v => v.is_boolean() }
impl_eq! { for Boolean }
impl_hash! { for Boolean use get_hash }
impl_partial_eq! { Data for Boolean use identity }
impl_partial_eq! { Value for Boolean use identity }
impl_partial_eq! { Primitive for Boolean use identity }
impl_partial_eq! { Boolean for Boolean use identity }

/// A superclass for symbols and strings.
#[repr(C)]
#[derive(Debug)]
pub struct Name(Opaque);

impl_deref! { Primitive for Name }
impl_try_from! { Data for Name if v => v.is_name() }
impl_try_from! { Value for Name if v => v.is_name() }
impl_try_from! { Primitive for Name if v => v.is_name() }
impl_from! { String for Name }
impl_from! { Symbol for Name }
impl_eq! { for Name }
impl_hash! { for Name use get_identity_hash }
impl_partial_eq! { Value for Name use same_value_zero }
impl_partial_eq! { Primitive for Name use same_value_zero }
impl_partial_eq! { Name for Name use strict_equals }
impl_partial_eq! { String for Name use strict_equals }
impl_partial_eq! { Symbol for Name use identity }

/// A JavaScript string value (ECMA-262, 4.3.17).
#[repr(C)]
#[derive(Debug)]
pub struct String(Opaque);

impl_deref! { Name for String }
impl_try_from! { Data for String if v => v.is_string() }
impl_try_from! { Value for String if v => v.is_string() }
impl_try_from! { Primitive for String if v => v.is_string() }
impl_try_from! { Name for String if v => v.is_string() }
impl_eq! { for String }
impl_hash! { for String use get_identity_hash }
impl_partial_eq! { Value for String use same_value_zero }
impl_partial_eq! { Primitive for String use same_value_zero }
impl_partial_eq! { Name for String use strict_equals }
impl_partial_eq! { String for String use strict_equals }

/// A JavaScript symbol (ECMA-262 edition 6)
#[repr(C)]
#[derive(Debug)]
pub struct Symbol(Opaque);

impl_deref! { Name for Symbol }
impl_try_from! { Data for Symbol if v => v.is_symbol() }
impl_try_from! { Value for Symbol if v => v.is_symbol() }
impl_try_from! { Primitive for Symbol if v => v.is_symbol() }
impl_try_from! { Name for Symbol if v => v.is_symbol() }
impl_eq! { for Symbol }
impl_hash! { for Symbol use get_identity_hash }
impl_partial_eq! { Data for Symbol use identity }
impl_partial_eq! { Value for Symbol use identity }
impl_partial_eq! { Primitive for Symbol use identity }
impl_partial_eq! { Name for Symbol use identity }
impl_partial_eq! { Symbol for Symbol use identity }
