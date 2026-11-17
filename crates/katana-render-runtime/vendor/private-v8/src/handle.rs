
#[path = "handle_global.rs"]
mod global;
#[path = "handle_local.rs"]
mod local;

pub use global::Global;
pub use local::Local;

#[path = "handle_access.rs"]
mod access;
#[path = "handle_identity.rs"]
mod identity;
#[path = "handle_metadata.rs"]
mod metadata;

pub use access::Handle;
use metadata::HandleHost;
pub use metadata::HandleInfo;

#[path = "handle_native.rs"]
mod native;

use native::{v8__Global__New, v8__Global__Reset, v8__Local__New};
