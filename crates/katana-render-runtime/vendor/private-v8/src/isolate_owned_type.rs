use std::ptr::NonNull;

use super::RealIsolate;

/// Same as Isolate but gets disposed when it goes out of scope.
#[derive(Debug)]
pub struct OwnedIsolate {
  pub(super) cxx_isolate: NonNull<RealIsolate>,
}
