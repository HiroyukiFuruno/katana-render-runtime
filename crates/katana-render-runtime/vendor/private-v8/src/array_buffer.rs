/* Copyright 2019-2026 the Deno authors. MIT license. */

#[path = "array_buffer_allocator.rs"]
mod allocator;

pub use allocator::Allocator;
