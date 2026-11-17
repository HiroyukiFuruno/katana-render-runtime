use super::{
  Data, v8__Data__IsBigInt, v8__Data__IsBoolean, v8__Data__IsContext,
  v8__Data__IsFixedArray, v8__Data__IsFunctionTemplate, v8__Data__IsModule,
  v8__Data__IsModuleRequest, v8__Data__IsName, v8__Data__IsNumber,
  v8__Data__IsObjectTemplate, v8__Data__IsPrimitive, v8__Data__IsPrivate,
  v8__Data__IsString, v8__Data__IsSymbol, v8__Data__IsValue,
};

impl Data {
  /// Returns true if this data is a `BigInt`.
  #[inline(always)]
  pub fn is_big_int(&self) -> bool {
    unsafe { v8__Data__IsBigInt(self) }
  }

  /// Returns true if this data is a `Boolean`.
  #[inline(always)]
  pub fn is_boolean(&self) -> bool {
    unsafe { v8__Data__IsBoolean(self) }
  }

  /// Returns true if this data is a `Context`.
  #[inline(always)]
  pub fn is_context(&self) -> bool {
    unsafe { v8__Data__IsContext(self) }
  }

  /// Returns true if this data is a `FixedArray`.
  #[inline(always)]
  pub fn is_fixed_array(&self) -> bool {
    unsafe { v8__Data__IsFixedArray(self) }
  }

  /// Returns true if this data is a `FunctionTemplate`.
  #[inline(always)]
  pub fn is_function_template(&self) -> bool {
    unsafe { v8__Data__IsFunctionTemplate(self) }
  }

  /// Returns true if this data is a `Module`.
  #[inline(always)]
  pub fn is_module(&self) -> bool {
    unsafe { v8__Data__IsModule(self) }
  }

  /// Returns true if this data is a `ModuleRequest`.
  #[inline(always)]
  pub fn is_module_request(&self) -> bool {
    unsafe { v8__Data__IsModuleRequest(self) }
  }

  /// Returns true if this data is a `Name`.
  #[inline(always)]
  pub fn is_name(&self) -> bool {
    unsafe { v8__Data__IsName(self) }
  }

  /// Returns true if this data is a `Number`.
  #[inline(always)]
  pub fn is_number(&self) -> bool {
    unsafe { v8__Data__IsNumber(self) }
  }

  /// Returns true if this data is a `ObjectTemplate`.
  #[inline(always)]
  pub fn is_object_template(&self) -> bool {
    unsafe { v8__Data__IsObjectTemplate(self) }
  }

  /// Returns true if this data is a `Primitive`.
  #[inline(always)]
  pub fn is_primitive(&self) -> bool {
    unsafe { v8__Data__IsPrimitive(self) }
  }

  /// Returns true if this data is a `Private`.
  #[inline(always)]
  pub fn is_private(&self) -> bool {
    unsafe { v8__Data__IsPrivate(self) }
  }

  /// Returns true if this data is a `String`.
  #[inline(always)]
  pub fn is_string(&self) -> bool {
    unsafe { v8__Data__IsString(self) }
  }

  /// Returns true if this data is a `Symbol`.
  #[inline(always)]
  pub fn is_symbol(&self) -> bool {
    unsafe { v8__Data__IsSymbol(self) }
  }

  /// Returns true if this data is a `Value`.
  #[inline(always)]
  pub fn is_value(&self) -> bool {
    unsafe { v8__Data__IsValue(self) }
  }
}
