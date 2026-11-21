use super::{Data, Opaque, Primitive, Value};

/// A JavaScript number value (ECMA-262, 4.3.20)
#[repr(C)]
#[derive(Debug)]
pub struct Number(Opaque);

impl_deref! { Primitive for Number }
impl_try_from! { Data for Number if v => v.is_number() }
impl_try_from! { Value for Number if v => v.is_number() }
impl_try_from! { Primitive for Number if v => v.is_number() }
impl_from! { Integer for Number }
impl_from! { Int32 for Number }
impl_from! { Uint32 for Number }
impl_eq! { for Number }
impl_hash! { for Number use get_hash }
impl_partial_eq! { Value for Number use same_value_zero }
impl_partial_eq! { Primitive for Number use same_value_zero }
impl_partial_eq! { Number for Number use same_value_zero }
impl_partial_eq! { Integer for Number use same_value_zero }
impl_partial_eq! { Int32 for Number use same_value_zero }
impl_partial_eq! { Uint32 for Number use same_value_zero }

/// A JavaScript value representing a signed integer.
#[repr(C)]
#[derive(Debug)]
pub struct Integer(Opaque);

impl_deref! { Number for Integer }
impl_try_from! { Data for Integer if v => v.is_number() && (cast::<Number>(v).is_int32() || cast::<Number>(v).is_uint32()) }
impl_try_from! { Value for Integer if v => v.is_int32() || v.is_uint32() }
impl_try_from! { Primitive for Integer if v => v.is_int32() || v.is_uint32() }
impl_try_from! { Number for Integer if v => v.is_int32() || v.is_uint32() }
impl_from! { Int32 for Integer }
impl_from! { Uint32 for Integer }
impl_eq! { for Integer }
impl_hash! { for Integer use get_hash }
impl_partial_eq! { Value for Integer use same_value_zero }
impl_partial_eq! { Primitive for Integer use same_value_zero }
impl_partial_eq! { Number for Integer use same_value_zero }
impl_partial_eq! { Integer for Integer use strict_equals }
impl_partial_eq! { Int32 for Integer use strict_equals }
impl_partial_eq! { Uint32 for Integer use strict_equals }

/// A JavaScript value representing a 32-bit signed integer.
#[repr(C)]
#[derive(Debug)]
pub struct Int32(Opaque);

impl_deref! { Integer for Int32 }
impl_try_from! { Data for Int32 if v => v.is_number() && cast::<Number>(v).is_int32() }
impl_try_from! { Value for Int32 if v => v.is_int32() }
impl_try_from! { Primitive for Int32 if v => v.is_int32() }
impl_try_from! { Number for Int32 if v => v.is_int32() }
impl_try_from! { Integer for Int32 if v => v.is_int32() }
impl_eq! { for Int32 }
impl_hash! { for Int32 use get_hash }
impl_partial_eq! { Value for Int32 use same_value_zero }
impl_partial_eq! { Primitive for Int32 use same_value_zero }
impl_partial_eq! { Number for Int32 use same_value_zero }
impl_partial_eq! { Integer for Int32 use strict_equals }
impl_partial_eq! { Int32 for Int32 use strict_equals }

/// A JavaScript value representing a 32-bit unsigned integer.
#[repr(C)]
#[derive(Debug)]
pub struct Uint32(Opaque);

impl_deref! { Integer for Uint32 }
impl_try_from! { Data for Uint32 if v => v.is_number() && cast::<Number>(v).is_uint32() }
impl_try_from! { Value for Uint32 if v => v.is_uint32() }
impl_try_from! { Primitive for Uint32 if v => v.is_uint32() }
impl_try_from! { Number for Uint32 if v => v.is_uint32() }
impl_try_from! { Integer for Uint32 if v => v.is_uint32() }
impl_eq! { for Uint32 }
impl_hash! { for Uint32 use get_hash }
impl_partial_eq! { Value for Uint32 use same_value_zero }
impl_partial_eq! { Primitive for Uint32 use same_value_zero }
impl_partial_eq! { Number for Uint32 use same_value_zero }
impl_partial_eq! { Integer for Uint32 use strict_equals }
impl_partial_eq! { Uint32 for Uint32 use strict_equals }
