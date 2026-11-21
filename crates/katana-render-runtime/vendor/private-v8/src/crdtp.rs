/* Copyright 2024 the Deno authors. All rights reserved. MIT license. */

#[path = "crdtp_dispatchable.rs"]
mod crdtp_dispatchable;
#[path = "crdtp_domain_dispatcher.rs"]
mod crdtp_domain_dispatcher;
#[path = "crdtp_fallthrough.rs"]
mod crdtp_fallthrough;
#[path = "crdtp_frontend_channel.rs"]
mod crdtp_frontend_channel;
#[path = "crdtp_serializable.rs"]
mod crdtp_serializable;

pub use crdtp_dispatchable::Dispatchable;
pub use crdtp_serializable::Serializable;

use crdtp_serializable::RawSerializable;

unsafe extern "C" {
  fn crdtp__Serializable__DELETE(this: *mut RawSerializable);
}
