/* Copyright 2019-2021 the Deno authors. All rights reserved. MIT license. */

use crate::private_v8::support::Opaque;

/// Represents the microtask queue, where microtasks are stored and processed.
/// https://html.spec.whatwg.org/multipage/webappapis.html#microtask-queue
/// https://html.spec.whatwg.org/multipage/webappapis.html#enqueuejob(queuename,-job,-arguments)
/// https://html.spec.whatwg.org/multipage/webappapis.html#perform-a-microtask-checkpoint
///
/// A MicrotaskQueue instance may be associated to multiple Contexts by passing
/// it to Context::New(), and they can be detached by Context::DetachGlobal().
///
/// Use the same instance of MicrotaskQueue for all Contexts that may access each
/// other synchronously. E.g. for Web embedding, use the same instance for all
/// origins that share the same URL scheme and eTLD+1.
#[repr(C)]
#[derive(Debug)]
pub struct MicrotaskQueue(Opaque);
