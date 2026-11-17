#[path = "platform_custom.rs"]
mod custom;
#[path = "platform_lifetime.rs"]
mod lifetime;
#[path = "platform_operations.rs"]
mod operations;
#[path = "platform_tasks.rs"]
mod tasks;

pub use lifetime::Platform;
pub use tasks::{IdleTask, Task};

use crate::private_v8::support::{int, UniqueRef};

unsafe extern "C" {
  fn v8__Platform__NewDefaultPlatform(
    thread_pool_size: int,
    idle_task_support: bool,
  ) -> *mut Platform;
}

impl Platform {
  const MAX_WORKER_THREADS: u32 = 16;
  /// Returns a new instance of the default v8::Platform implementation.
  ///
  /// |thread_pool_size| is the number of worker threads to allocate for
  /// background jobs. If a value of zero is passed, a suitable default
  /// based on the current number of processors online will be chosen.
  /// If |idle_task_support| is enabled then the platform will accept idle
  /// tasks (IdleTasksEnabled will return true) and will rely on the embedder
  /// calling v8::platform::RunIdleTasks to process the idle tasks.
  ///
  /// The default platform for v8 may include restrictions and caveats on thread
  /// creation and initialization. This platform should only be used in cases
  /// where v8 can be reliably initialized on the application's main thread, or
  /// the parent thread to all threads in the system that will use v8.
  ///
  /// One example of a restriction is the use of Memory Protection Keys (pkeys)
  /// on modern Linux systems using modern Intel/AMD processors. This particular
  /// technology requires that all threads using v8 are created as descendent
  /// threads of the thread that called `v8::Initialize`.
  #[inline(always)]
  pub fn new(
    thread_pool_size: u32,
    idle_task_support: bool,
  ) -> UniqueRef<Self> {
    unsafe {
      UniqueRef::from_raw(v8__Platform__NewDefaultPlatform(
        thread_pool_size.min(Self::MAX_WORKER_THREADS) as i32,
        idle_task_support,
      ))
    }
  }



}
