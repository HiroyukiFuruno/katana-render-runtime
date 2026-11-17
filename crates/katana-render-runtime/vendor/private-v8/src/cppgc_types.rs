use super::*;

/// Visitor passed to the native garbage collection callback.
#[repr(C)]
#[derive(Debug)]
pub struct Visitor(Opaque);
