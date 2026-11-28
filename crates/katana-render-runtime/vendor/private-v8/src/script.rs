use std::mem::MaybeUninit;
use std::ptr::null;

use crate::private_v8::Context;
use crate::private_v8::Data;
use crate::private_v8::Local;
use crate::private_v8::Script;
use crate::private_v8::String;
use crate::private_v8::Value;
use crate::private_v8::scope::PinScope;

/// The origin, within a file, of a script.
#[repr(C)]
#[derive(Debug)]
pub struct ScriptOrigin<'s>(
  [u8; crate::private_v8::binding::v8__ScriptOrigin_SIZE],
  /* require pointer alignment */
  [&'s (); 0],
);

unsafe extern "C" {
  fn v8__Script__Compile(
    context: *const Context,
    source: *const String,
    origin: *const ScriptOrigin,
  ) -> *const Script;
  fn v8__Script__Run(
    script: *const Script,
    context: *const Context,
  ) -> *const Value;

  fn v8__ScriptOrigin__CONSTRUCT(
    buf: *mut MaybeUninit<ScriptOrigin>,
    resource_name: *const Value,
    resource_line_offset: i32,
    resource_column_offset: i32,
    resource_is_shared_cross_origin: bool,
    script_id: i32,
    source_map_url: *const Value,
    resource_is_opaque: bool,
    is_wasm: bool,
    is_module: bool,
    host_defined_options: *const Data,
  );
}

impl Script {
  /// A shorthand for ScriptCompiler::Compile().
  #[inline(always)]
  pub fn compile<'s>(
    scope: &PinScope<'s, '_>,
    source: Local<String>,
    origin: Option<&ScriptOrigin>,
  ) -> Option<Local<'s, Script>> {
    unsafe {
      scope.cast_local(|sd| {
        v8__Script__Compile(
          sd.get_current_context(),
          &*source,
          origin.map_or_else(null, |r| r as *const _),
        )
      })
    }
  }

  /// Runs the script returning the resulting value. It will be run in the
  /// context in which it was created (ScriptCompiler::CompileBound or
  /// UnboundScript::BindToCurrentContext()).
  #[inline]
  pub fn run<'s>(&self, scope: &PinScope<'s, '_>) -> Option<Local<'s, Value>> {
    unsafe {
      scope.cast_local(|sd| v8__Script__Run(self, sd.get_current_context()))
    }
  }
}

/// The origin, within a file, of a script.
impl<'s> ScriptOrigin<'s> {
  #[allow(clippy::too_many_arguments)]
  #[inline(always)]
  pub fn new<'i>(
    /* TODO(littledivy): remove */
    _scope: &PinScope<'s, 'i>,
    resource_name: Local<'s, Value>,
    resource_line_offset: i32,
    resource_column_offset: i32,
    resource_is_shared_cross_origin: bool,
    script_id: i32,
    source_map_url: Option<Local<'s, Value>>,
    resource_is_opaque: bool,
    is_wasm: bool,
    is_module: bool,
    host_defined_options: Option<Local<'s, Data>>,
  ) -> Self {
    unsafe {
      let mut buf = std::mem::MaybeUninit::<ScriptOrigin>::uninit();
      v8__ScriptOrigin__CONSTRUCT(
        &mut buf,
        &*resource_name,
        resource_line_offset,
        resource_column_offset,
        resource_is_shared_cross_origin,
        script_id,
        source_map_url.map_or_else(null, |l| &*l as *const Value),
        resource_is_opaque,
        is_wasm,
        is_module,
        host_defined_options.map_or_else(null, |l| &*l as *const Data),
      );
      buf.assume_init()
    }
  }


}
