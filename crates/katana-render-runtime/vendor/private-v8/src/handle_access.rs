use std::borrow::Borrow;

use super::{Global, HandleHost, HandleInfo, Local};

pub trait Handle: Sized {
  type Data;

  #[doc(hidden)]
  fn get_handle_info(&self) -> HandleInfo<Self::Data>;

  #[doc(hidden)]
  fn assert_safe_to_access(&self) {}

}

impl<T> Handle for Local<'_, T> {
  type Data = T;
  fn get_handle_info(&self) -> HandleInfo<T> {
    HandleInfo::new(self.as_non_null(), HandleHost::Scope)
  }
}

impl<'a, 's: 'a, T> Handle for &'a Local<'s, T> {
  type Data = T;
  fn get_handle_info(&self) -> HandleInfo<T> {
    HandleInfo::new(self.as_non_null(), HandleHost::Scope)
  }
}

impl<T> Handle for Global<T> {
  type Data = T;
  fn get_handle_info(&self) -> HandleInfo<T> {
    HandleInfo::new(self.data, self.get_handle_host())
  }
  fn assert_safe_to_access(&self) {
    self.assert_access_allowed();
  }
}

impl<T> Handle for &Global<T> {
  type Data = T;
  fn get_handle_info(&self) -> HandleInfo<T> {
    HandleInfo::new(self.data, self.get_handle_host())
  }
  fn assert_safe_to_access(&self) {
    self.assert_access_allowed();
  }
}

impl<T> Borrow<T> for Local<'_, T> {
  fn borrow(&self) -> &T {
    self
  }
}

/* `Borrow<T> for Global<T>` is deliberately absent. `fn borrow(&self) -> &T` */
/* has nowhere to take proof that the caller may touch the isolate, and */
/* nowhere to tie the returned reference's lifetime to that proof: any check */
/* it made would expire while the `&T` it handed out stayed alive. Heap-object */
/* wrappers are `!Sync`, which prevents moving that reference to another */
/* thread, but does not prevent it from outliving the isolate. Use */
/* `Local::new(scope, &global)` instead — a `Local` is bound to a scope, which */
/* is bound to the isolate. */
