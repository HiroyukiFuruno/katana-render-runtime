use super::{Data, Opaque};

/// An AccessorSignature specifies which receivers are valid parameters
/// to an accessor callback.
#[repr(C)]
#[derive(Debug)]
pub struct AccessorSignature(Opaque);

impl_deref! { Data for AccessorSignature }
impl_eq! { for AccessorSignature }
impl_partial_eq! { Data for AccessorSignature use identity }
impl_partial_eq! { AccessorSignature for AccessorSignature use identity }

/// A sandboxed execution context with its own set of built-in objects
/// and functions.
#[repr(C)]
#[derive(Debug)]
pub struct Context(Opaque);

impl_deref! { Data for Context }
impl_try_from! { Data for Context if v => v.is_context() }
impl_eq! { for Context }
impl_partial_eq! { Data for Context use identity }
impl_partial_eq! { Context for Context use identity }

/// A fixed-sized array with elements of type Data.
#[repr(C)]
#[derive(Debug)]
pub struct FixedArray(Opaque);

impl_deref! { Data for FixedArray }
impl_try_from! { Data for FixedArray if v => v.is_fixed_array() }
impl_eq! { for FixedArray }
impl_partial_eq! { Data for FixedArray use identity }
impl_partial_eq! { FixedArray for FixedArray use identity }

/// An error message.
#[repr(C)]
#[derive(Debug)]
pub struct Message(Opaque);

impl_eq! { for Message }
impl_partial_eq! { Message for Message use identity }

/// A compiled JavaScript module.
#[repr(C)]
#[derive(Debug)]
pub struct Module(Opaque);

impl_deref! { Data for Module }
impl_try_from! { Data for Module if v => v.is_module() }
impl_eq! { for Module }
impl_hash! { for Module use get_identity_hash }
impl_partial_eq! { Data for Module use identity }
impl_partial_eq! { Module for Module use identity }

#[repr(C)]
#[derive(Debug)]
pub struct ModuleRequest(Opaque);

impl_deref! { Data for ModuleRequest }
impl_try_from! { Data for ModuleRequest if v => v.is_module_request() }
impl_eq! { for ModuleRequest }
impl_partial_eq! { Data for ModuleRequest use identity }
impl_partial_eq! { ModuleRequest for ModuleRequest use identity }

/// A private symbol
///
/// This is an experimental feature. Use at your own risk.
#[repr(C)]
#[derive(Debug)]
pub struct Private(Opaque);

impl_deref! { Data for Private }
impl_try_from! { Data for Private if v => v.is_private() }
impl_eq! { for Private }
impl_partial_eq! { Data for Private use identity }
impl_partial_eq! { Private for Private use identity }

/// A compiled JavaScript script, tied to a Context which was active when the
/// script was compiled.
#[repr(C)]
#[derive(Debug)]
pub struct Script(Opaque);

impl_eq! { for Script }
impl_partial_eq! { Script for Script use identity }
