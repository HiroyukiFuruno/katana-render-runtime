/* Copyright 2019-2022 the Deno authors. All rights reserved. MIT license. */

use std::cell::Cell;
use std::marker::PhantomData;

/* V8 heap objects may only be accessed on their isolate's thread (or on the */
/* thread currently holding its Locker). Keep the opaque payload `Send` but not */
/* `Sync`, so a shared reference obtained from a Local cannot cross threads. */
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Opaque {
  _ffi: [u8; 0],
  _not_sync: PhantomData<Cell<()>>,
}

unsafe extern "C" {
  fn v8__Data__EQ(this: *const Data, other: *const Data) -> bool;
  fn v8__Data__IsBigInt(this: *const Data) -> bool;
  fn v8__Data__IsBoolean(this: *const Data) -> bool;
  fn v8__Data__IsContext(this: *const Data) -> bool;
  fn v8__Data__IsFixedArray(this: *const Data) -> bool;
  fn v8__Data__IsFunctionTemplate(this: *const Data) -> bool;
  fn v8__Data__IsModule(this: *const Data) -> bool;
  fn v8__Data__IsModuleRequest(this: *const Data) -> bool;
  fn v8__Data__IsName(this: *const Data) -> bool;
  fn v8__Data__IsNumber(this: *const Data) -> bool;
  fn v8__Data__IsObjectTemplate(this: *const Data) -> bool;
  fn v8__Data__IsPrimitive(this: *const Data) -> bool;
  fn v8__Data__IsPrivate(this: *const Data) -> bool;
  fn v8__Data__IsString(this: *const Data) -> bool;
  fn v8__Data__IsSymbol(this: *const Data) -> bool;
  fn v8__Data__IsValue(this: *const Data) -> bool;

  fn v8__Value__SameValue(this: *const Value, other: *const Value) -> bool;
  fn v8__Value__StrictEquals(this: *const Value, other: *const Value) -> bool;
}

#[macro_use]
#[path = "data_conversions.rs"]
mod conversions;

#[path = "data_type_queries.rs"]
mod type_queries;

#[path = "data_base.rs"]
mod base;
pub use base::*;

#[path = "data_metadata.rs"]
mod metadata;
pub use metadata::*;

#[path = "data_templates.rs"]
mod templates;
pub use templates::*;

#[path = "data_values.rs"]
mod values;
pub use values::*;

#[path = "data_objects.rs"]
mod objects;
pub use objects::*;

#[path = "data_array_views.rs"]
mod array_views;
pub use array_views::*;

#[path = "data_big_and_float_arrays.rs"]
mod big_and_float_arrays;
pub use big_and_float_arrays::*;

#[path = "data_integer_arrays.rs"]
mod integer_arrays;
pub use integer_arrays::*;

#[path = "data_boxed_primitives.rs"]
mod boxed_primitives;
pub use boxed_primitives::*;

#[path = "data_builtin_objects.rs"]
mod builtin_objects;
pub use builtin_objects::*;

#[path = "data_primitives.rs"]
mod primitives;
pub use primitives::*;

#[path = "data_numbers.rs"]
mod numbers;
pub use numbers::*;

#[path = "data_error.rs"]
mod error;
pub use error::DataError;
