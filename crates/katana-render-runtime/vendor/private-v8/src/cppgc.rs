/* Copyright 2019-2021 the Deno authors. All rights reserved. MIT license */

use crate::private_v8::binding::RustObj;
use crate::private_v8::support::Opaque;
use std::ffi::CStr;
use std::ffi::c_char;
#[path = "cppgc_callbacks.rs"]
mod cppgc_callbacks;
#[path = "cppgc_heap.rs"]
mod cppgc_heap;
#[path = "cppgc_objects.rs"]
mod cppgc_objects;
#[path = "cppgc_types.rs"]
mod cppgc_types;

use cppgc_objects::{get_rust_obj, get_rust_obj_mut};

pub use cppgc_heap::Heap;
use cppgc_types::Visitor;
