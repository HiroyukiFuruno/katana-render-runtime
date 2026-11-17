/* Copyright 2019-2021 the Deno authors. All rights reserved. MIT license. */
use crate::private_v8::platform::Platform;
use crate::private_v8::support::SharedRef;
use std::sync::Mutex;

unsafe extern "C" {
  fn v8__V8__InitializePlatform(platform: *mut Platform);
  fn v8__V8__Initialize();
}

#[derive(Debug)]
enum GlobalState {
  Uninitialized,
  PlatformInitialized(SharedRef<Platform>),
  Initialized(SharedRef<Platform>),
}
use GlobalState::*;

static GLOBAL_STATE: Mutex<GlobalState> = Mutex::new(Uninitialized);

pub struct V8;

impl V8 {
  pub fn assert_initialized() {
    let global_state_guard = GLOBAL_STATE.lock().unwrap();
    match *global_state_guard {
      Initialized(_) => {}
      _ => panic!("Invalid global state"),
    };
  }

  /// Sets the v8::Platform to use. This should be invoked before V8 is
  /// initialized.
  pub fn initialize_platform(platform: SharedRef<Platform>) {
    let mut global_state_guard = GLOBAL_STATE.lock().unwrap();
    *global_state_guard = match *global_state_guard {
      Uninitialized => PlatformInitialized(platform.clone()),
      _ => panic!("Invalid global state"),
    };

    {
      unsafe {
        v8__V8__InitializePlatform(&*platform as *const Platform as *mut _);
      };
    }
  }

  /// Initializes V8. This function needs to be called before the first Isolate
  /// is created. It always returns true.
  ///
  /// # Call this before creating threads that will enter an isolate
  ///
  /// On platforms with memory protection keys (x86-64 with `pku`/`ospke`,
  /// which covers most current server hardware), V8 allocates a protection
  /// key here and uses it to guard its internal pointer tables. Access to a
  /// protection key is governed by the `PKRU` register, which is **per
  /// thread** and is **inherited from the creating thread at thread creation
  /// time**.
  ///
  /// A thread created *before* this call therefore never gains access to that
  /// key. Such a thread runs correctly until it enters an isolate, at which
  /// point its first read of a pointer table traps with `SIGSEGV` and
  /// `si_code == SEGV_PKUERR`. The faulting address lies in a mapping that
  /// `/proc/<pid>/maps` reports as present and readable, because protection
  /// keys are enforced by the MMU and appear in no mapping flag.
  ///
  /// This matters most for embedders that build a thread pool up front and
  /// then create isolates on its workers:
  ///
  /// Initialize the V8 platform and V8 before creating the worker threads
  /// that may enter an isolate.
  ///
  /// Creating the pool first compiles and runs, and fails only once a worker
  /// first executes JavaScript. V8 states the same requirement internally, in
  /// the name of `SandboxHardwareSupport::TryActivateBeforeThreadCreation`.
  pub fn initialize() {
    let mut global_state_guard = GLOBAL_STATE.lock().unwrap();
    *global_state_guard = match *global_state_guard {
      PlatformInitialized(ref platform) => Initialized(platform.clone()),
      _ => panic!("Invalid global state"),
    };
    unsafe { v8__V8__Initialize() }
  }

  /// Sets the v8::Platform to use. This should be invoked before V8 is
  /// initialized.
  pub fn get_current_platform() -> SharedRef<Platform> {
    let global_state_guard = GLOBAL_STATE.lock().unwrap();
    match *global_state_guard {
      Initialized(ref platform) => platform.clone(),
      _ => panic!("Invalid global state"),
    }
  }
}

#[cfg(test)]
mod test_sandbox_use {
  /* __IsSandboxEnabled is used for testing that sandbox is actually */
  /* enabled and is not a stable API */
  unsafe extern "C" {
    fn v8__V8__IsSandboxEnabled() -> bool;
  }

  fn is_sandboxed() -> bool {
    unsafe { v8__V8__IsSandboxEnabled() }
  }

  #[test]
  #[cfg(any())]
  fn test_sandbox_on() {
    assert!(is_sandboxed());
  }

  #[test]
  #[cfg(not(any()))]
  fn test_sandbox_off() {
    assert!(!is_sandboxed());
  }
}
