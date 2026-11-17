use crate::private_v8::Array;
use crate::private_v8::Context;
use crate::private_v8::Local;
use crate::private_v8::Name;
use crate::private_v8::Object;
use crate::private_v8::Private;
use crate::private_v8::PropertyAttribute;
use crate::private_v8::Value;
use crate::private_v8::isolate::RealIsolate;
use crate::private_v8::scope::PinScope;
use crate::private_v8::support::MaybeBool;
use crate::private_v8::support::int;
use std::num::NonZeroI32;

#[path = "object_arrays.rs"]
mod arrays;
#[path = "object_creation.rs"]
mod creation;
#[path = "object_definitions.rs"]
mod definitions;
#[cfg(test)]
#[path = "object_descriptors.rs"]
mod descriptors;
#[cfg(test)]
#[path = "object_enumeration.rs"]
mod enumeration;
#[path = "object_getters.rs"]
mod getters;
#[path = "object_private_properties.rs"]
mod private_properties;
#[path = "object_setters.rs"]
mod setters;

/// Object integrity levels can be used to restrict what can be done to an
/// object's properties.
#[cfg(test)]
#[derive(Debug)]
#[repr(C)]
pub enum IntegrityLevel {
  /// Frozen objects are like Sealed objects, except all existing properties are
  /// also made non-writable.
  Frozen,
  /// Sealed objects prevent addition of any new property on the object, makes
  /// all existing properties non-configurable, meaning they cannot be deleted,
  /// have their enumerability, configurability, or writability changed.
  Sealed,
}
