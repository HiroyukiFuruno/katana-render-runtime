use crate::private_v8::Value;

unsafe extern "C" {
  #[cfg(test)]
  fn v8__Value__IsUndefined(this: *const Value) -> bool;
  fn v8__Value__IsTrue(this: *const Value) -> bool;
  #[cfg(test)]
  fn v8__Value__IsFalse(this: *const Value) -> bool;
  fn v8__Value__IsName(this: *const Value) -> bool;
  fn v8__Value__IsString(this: *const Value) -> bool;
  fn v8__Value__IsSymbol(this: *const Value) -> bool;
  fn v8__Value__IsFunction(this: *const Value) -> bool;
  fn v8__Value__IsArray(this: *const Value) -> bool;
  fn v8__Value__IsObject(this: *const Value) -> bool;
  fn v8__Value__IsBigInt(this: *const Value) -> bool;
  fn v8__Value__IsBoolean(this: *const Value) -> bool;
  fn v8__Value__IsNumber(this: *const Value) -> bool;
  fn v8__Value__IsExternal(this: *const Value) -> bool;
  fn v8__Value__IsInt32(this: *const Value) -> bool;
  fn v8__Value__IsUint32(this: *const Value) -> bool;
}

impl Value {
  /// Returns true if this value is the undefined value.  See ECMA-262 4.3.10.
  #[inline(always)]
  #[cfg(test)]
  pub fn is_undefined(&self) -> bool {
    unsafe { v8__Value__IsUndefined(self) }
  }

  /// Returns true if this value is true.
  /// This is not the same as `BooleanValue()`. The latter performs a
  /// conversion to boolean, i.e. the result of `Boolean(value)` in JS, whereas
  /// this checks `value === true`.
  #[inline(always)]
  pub fn is_true(&self) -> bool {
    unsafe { v8__Value__IsTrue(self) }
  }

  /// Returns true if this value is false.
  /// This is not the same as `!BooleanValue()`. The latter performs a
  /// conversion to boolean, i.e. the result of `!Boolean(value)` in JS, whereas
  /// this checks `value === false`.
  #[cfg(test)]
  #[inline(always)]
  pub fn is_false(&self) -> bool {
    unsafe { v8__Value__IsFalse(self) }
  }

  /// Returns true if this value is a symbol or a string.
  /// This is equivalent to
  /// `typeof value === 'string' || typeof value === 'symbol'` in JS.
  #[inline(always)]
  pub fn is_name(&self) -> bool {
    unsafe { v8__Value__IsName(self) }
  }

  /// Returns true if this value is an instance of the String type.
  /// See ECMA-262 8.4.
  #[inline(always)]
  pub fn is_string(&self) -> bool {
    unsafe { v8__Value__IsString(self) }
  }

  /// Returns true if this value is a symbol.
  /// This is equivalent to `typeof value === 'symbol'` in JS.
  #[inline(always)]
  pub fn is_symbol(&self) -> bool {
    unsafe { v8__Value__IsSymbol(self) }
  }

  /// Returns true if this value is a function.
  #[inline(always)]
  pub fn is_function(&self) -> bool {
    unsafe { v8__Value__IsFunction(self) }
  }

  /// Returns true if this value is an array. Note that it will return false for
  /// an Proxy for an array.
  #[inline(always)]
  pub fn is_array(&self) -> bool {
    unsafe { v8__Value__IsArray(self) }
  }

  /// Returns true if this value is an object.
  #[inline(always)]
  pub fn is_object(&self) -> bool {
    unsafe { v8__Value__IsObject(self) }
  }

  /// Returns true if this value is a bigint.
  /// This is equivalent to `typeof value === 'bigint'` in JS.
  #[inline(always)]
  pub fn is_big_int(&self) -> bool {
    unsafe { v8__Value__IsBigInt(self) }
  }

  /// Returns true if this value is boolean.
  /// This is equivalent to `typeof value === 'boolean'` in JS.
  #[inline(always)]
  pub fn is_boolean(&self) -> bool {
    unsafe { v8__Value__IsBoolean(self) }
  }

  /// Returns true if this value is a number.
  #[inline(always)]
  pub fn is_number(&self) -> bool {
    unsafe { v8__Value__IsNumber(self) }
  }

  /// Returns true if this value is an `External` object.
  #[inline(always)]
  pub fn is_external(&self) -> bool {
    unsafe { v8__Value__IsExternal(self) }
  }

  /// Returns true if this value is a 32-bit signed integer.
  #[inline(always)]
  pub fn is_int32(&self) -> bool {
    unsafe { v8__Value__IsInt32(self) }
  }

  /// Returns true if this value is a 32-bit unsigned integer.
  #[inline(always)]
  pub fn is_uint32(&self) -> bool {
    unsafe { v8__Value__IsUint32(self) }
  }
}
