use super::{
  AllowJavascriptExecutionScope,
  CallbackScope,
  ClearCachedContext,
  Context,
  ContextScope,
  DisallowJavascriptExecutionScope,
  EscapableHandleScope,
  FunctionCallbackInfo,
  GetIsolate,
  HandleScope,
  Isolate,
  Local,
  Message,
  OwnedIsolate,
  RealIsolate,
  TryCatch,
  raw,
};

mod get_isolate_impls {
  use crate::private_v8::{Promise, PromiseRejectMessage};

  use super::*;
  impl GetIsolate for Isolate {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      self.as_real_ptr()
    }
  }

  impl GetIsolate for OwnedIsolate {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      self.as_real_ptr()
    }
  }

  impl GetIsolate for FunctionCallbackInfo {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      self.get_isolate_ptr()
    }
  }

  impl GetIsolate for Local<'_, Context> {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      unsafe { raw::v8__Isolate__GetCurrent() }
    }
  }

  impl GetIsolate for Local<'_, Message> {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      unsafe { raw::v8__Isolate__GetCurrent() }
    }
  }

  impl GetIsolate for Local<'_, Promise> {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      unsafe { raw::v8__Isolate__GetCurrent() }
    }
  }

  impl GetIsolate for PromiseRejectMessage<'_> {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      unsafe { raw::v8__Isolate__GetCurrent() }
    }
  }

  impl<C> GetIsolate for HandleScope<'_, C> {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      self.isolate.as_ptr()
    }
  }

  impl<P: GetIsolate + ClearCachedContext> GetIsolate
    for ContextScope<'_, '_, P>
  {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      self.scope.get_isolate_ptr()
    }
  }

  impl<P: GetIsolate> GetIsolate for &mut P {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      P::get_isolate_ptr(self)
    }
  }

  impl<P: GetIsolate> GetIsolate for &P {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      P::get_isolate_ptr(self)
    }
  }

  impl<P: GetIsolate> GetIsolate for TryCatch<'_, '_, P> {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      self.scope.get_isolate_ptr()
    }
  }

  impl<C> GetIsolate for CallbackScope<'_, C> {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      self.isolate.as_ptr()
    }
  }

  impl<C> GetIsolate for EscapableHandleScope<'_, '_, C> {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      self.isolate.as_ptr()
    }
  }

  impl<P: GetIsolate> GetIsolate for AllowJavascriptExecutionScope<'_, '_, P> {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      self.scope.get_isolate_ptr()
    }
  }

  impl<P: GetIsolate> GetIsolate for DisallowJavascriptExecutionScope<'_, '_, P> {
    fn get_isolate_ptr(&self) -> *mut RealIsolate {
      self.scope.get_isolate_ptr()
    }
  }
}
