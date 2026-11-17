use crate::private_v8::Value;

unsafe extern "C" {
  fn v8__Value__IsDate(this: *const Value) -> bool;
  fn v8__Value__IsBigIntObject(this: *const Value) -> bool;
  fn v8__Value__IsBooleanObject(this: *const Value) -> bool;
  fn v8__Value__IsNumberObject(this: *const Value) -> bool;
  fn v8__Value__IsStringObject(this: *const Value) -> bool;
  fn v8__Value__IsSymbolObject(this: *const Value) -> bool;
  fn v8__Value__IsRegExp(this: *const Value) -> bool;
  fn v8__Value__IsPromise(this: *const Value) -> bool;
  fn v8__Value__IsMap(this: *const Value) -> bool;
  fn v8__Value__IsSet(this: *const Value) -> bool;
}

impl Value {
  /// Returns true if this value is a Date.
  #[inline(always)]
  pub fn is_date(&self) -> bool {
    unsafe { v8__Value__IsDate(self) }
  }


  /// Returns true if this value is a BigInt object.
  #[inline(always)]
  pub fn is_big_int_object(&self) -> bool {
    unsafe { v8__Value__IsBigIntObject(self) }
  }

  /// Returns true if this value is a Boolean object.
  #[inline(always)]
  pub fn is_boolean_object(&self) -> bool {
    unsafe { v8__Value__IsBooleanObject(self) }
  }

  /// Returns true if this value is a Number object.
  #[inline(always)]
  pub fn is_number_object(&self) -> bool {
    unsafe { v8__Value__IsNumberObject(self) }
  }

  /// Returns true if this value is a String object.
  #[inline(always)]
  pub fn is_string_object(&self) -> bool {
    unsafe { v8__Value__IsStringObject(self) }
  }

  /// Returns true if this value is a Symbol object.
  #[inline(always)]
  pub fn is_symbol_object(&self) -> bool {
    unsafe { v8__Value__IsSymbolObject(self) }
  }


  /// Returns true if this value is a RegExp.
  #[inline(always)]
  pub fn is_reg_exp(&self) -> bool {
    unsafe { v8__Value__IsRegExp(self) }
  }



  /// Returns true if this value is a Promise.
  #[inline(always)]
  pub fn is_promise(&self) -> bool {
    unsafe { v8__Value__IsPromise(self) }
  }

  /// Returns true if this value is a Map.
  #[inline(always)]
  pub fn is_map(&self) -> bool {
    unsafe { v8__Value__IsMap(self) }
  }

  /// Returns true if this value is a Set.
  #[inline(always)]
  pub fn is_set(&self) -> bool {
    unsafe { v8__Value__IsSet(self) }
  }





}
