use super::{
  Pin,
  ScopeStorage,
};

pub trait Scope: Sized + sealed::Sealed + ScopeInit {}

pub(super) mod sealed {
  pub trait Sealed {}
}

pub trait ScopeInit: Sized {
  fn init_stack(storage: Pin<&mut ScopeStorage<Self>>) -> Pin<&mut Self>;

  unsafe fn deinit(me: &mut Self);
}
mod get_isolate {
  use crate::private_v8::RealIsolate;
  pub trait GetIsolate {
    fn get_isolate_ptr(&self) -> *mut RealIsolate;
  }
}
pub(crate) use get_isolate::GetIsolate;
pub trait NewHandleScope<'s> {
  type NewScope: Scope;

  fn make_new_scope(me: &'s mut Self) -> Self::NewScope;
}
mod new_context_scope {

  use super::{GetIsolate, Scope};
  use crate::private_v8::{Context, Local};

  pub trait NewContextScope<'s, 'c>: GetIsolate {
    type NewScope: Scope;

    fn make_new_scope(
      me: &'s mut Self,
      context: Local<'c, Context>,
    ) -> Self::NewScope;
  }
}
pub(super) use new_context_scope::NewContextScope;
mod clear_cached_context {
  pub trait ClearCachedContext {
    fn clear_cached_context(&self);
  }
}
pub(super) use clear_cached_context::ClearCachedContext;
