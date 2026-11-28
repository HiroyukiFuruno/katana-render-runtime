use super::*;
use crate::private_v8::support::Opaque;

/// Trait for implementing a domain-specific protocol dispatcher.
///
/// The `dispatch` method executes the command immediately and returns whether
/// the command was handled.
pub trait DomainDispatcherImpl {
  fn dispatch(
    &mut self,
    command: &[u8],
    dispatchable: &Dispatchable,
    handle: &DomainDispatcherHandle,
  ) -> bool;
}

/// Opaque handle supplied during domain dispatch.
#[repr(C)]
pub(super) struct RawDomainDispatcher(Opaque);

pub struct DomainDispatcherHandle {
  _ptr: *mut RawDomainDispatcher,
}

/// A domain dispatcher that delegates to a Rust `DomainDispatcherImpl`.
///
/// Ownership model: the Rust `DomainDispatcher` is heap-allocated and its
/// pointer is stored in the C++ `crdtp__DomainDispatcher__BASE`. When the
/// C++ side is destroyed, it calls back
/// into Rust via `crdtp__DomainDispatcher__BASE__Drop` to free the Rust
/// allocation.
struct DomainDispatcherData {
  ptr: *mut RawDomainDispatcher,
  imp: Box<dyn DomainDispatcherImpl>,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn crdtp__DomainDispatcher__BASE__Dispatch(
  rust_dispatcher: *mut std::ffi::c_void,
  command_data: *const u8,
  command_len: usize,
  dispatchable: &Dispatchable,
) -> bool {
  unsafe {
    let dd = &mut *(rust_dispatcher as *mut DomainDispatcherData);
    let command = std::slice::from_raw_parts(command_data, command_len);
    let handle = DomainDispatcherHandle { _ptr: dd.ptr };
    dd.imp.dispatch(command, dispatchable, &handle)
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn crdtp__DomainDispatcher__BASE__Drop(
  rust_dispatcher: *mut std::ffi::c_void,
) {
  unsafe {
    drop(Box::from_raw(rust_dispatcher as *mut DomainDispatcherData));
  }
}
