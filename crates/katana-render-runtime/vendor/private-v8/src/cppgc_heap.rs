use super::*;

#[repr(C)]
#[derive(Debug)]
/// A heap for allocating managed C++ objects.
///
/// Similar to v8::Isolate, the heap may only be accessed from one thread at a
/// time.

pub struct Heap(Opaque);
