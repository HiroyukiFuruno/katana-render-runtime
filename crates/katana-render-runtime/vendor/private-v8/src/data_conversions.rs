macro_rules! impl_deref {
  { $target:ident for $type:ident } => {
    impl std::ops::Deref for $type {
      type Target = $target;
      #[inline]
      fn deref(&self) -> &Self::Target {
        unsafe { &*(self as *const _ as *const Self::Target) }
      }
    }
  };
}

macro_rules! impl_from {
  { $source:ident for $type:ident } => {
    impl<'s> From<$crate::private_v8::Local<'s, $source>> for $crate::private_v8::Local<'s, $type> {
      #[inline]
      fn from(l: $crate::private_v8::Local<'s, $source>) -> Self {
        unsafe { std::mem::transmute(l) }
      }
    }
  };
}

macro_rules! impl_try_from {
  { $source:ident for $target:ident if $value:pat => $check:expr } => {
    impl<'s> TryFrom<$crate::private_v8::Local<'s, $source>> for $crate::private_v8::Local<'s, $target> {
      type Error = $crate::private_v8::data::DataError;
      #[inline]
      fn try_from(l: $crate::private_v8::Local<'s, $source>) -> Result<Self, Self::Error> {
        /* Not dead: `cast()` is sometimes used in the $check expression. */
        #[allow(dead_code)]
        #[inline(always)]
        fn cast<T>(l: $crate::private_v8::Local<$source>) -> $crate::private_v8::Local<T> {
          unsafe { std::mem::transmute::<$crate::private_v8::Local<$source>, $crate::private_v8::Local<T>>(l) }
        }
        match l {
          $value if $check => Ok(unsafe {
            std::mem::transmute::<$crate::private_v8::Local<'s, $source>, $crate::private_v8::Local<'s, $target>>(l)
          }),
          _ => Err($crate::private_v8::data::DataError::bad_type::<$target, $source>())
        }
      }
    }
  };
}

macro_rules! impl_eq {
  { for $type:ident } => {
    impl Eq for $type {}
  };
}

macro_rules! impl_hash {
  { for $type:ident use $method:ident } => {
    impl std::hash::Hash for $type {
      fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.$method(), state)
      }
    }
  };
}

macro_rules! impl_partial_eq {
  { $rhs:ident for $type:ident use identity } => {
    impl<'s> PartialEq<$rhs> for $type {
      #[inline]
      fn eq(&self, other: &$rhs) -> bool {
        let a = self as *const _ as *const $crate::private_v8::data::Data;
        let b = other as *const _ as *const $crate::private_v8::data::Data;
        unsafe { $crate::private_v8::data::v8__Data__EQ(a, b) }
      }
    }
  };
  { $rhs:ident for $type:ident use strict_equals } => {
    impl<'s> PartialEq<$rhs> for $type {
      #[inline]
      fn eq(&self, other: &$rhs) -> bool {
        let a = self as *const _ as *const $crate::private_v8::data::Value;
        let b = other as *const _ as *const $crate::private_v8::data::Value;
        unsafe { $crate::private_v8::data::v8__Value__StrictEquals(a, b) }
      }
    }
  };

  { $rhs:ident for $type:ident use same_value_zero } => {
    impl<'s> PartialEq<$rhs> for $type {
      #[inline]
      fn eq(&self, other: &$rhs) -> bool {
        let a = self as *const _ as *const $crate::private_v8::data::Value;
        let b = other as *const _ as *const $crate::private_v8::data::Value;
        unsafe {
          $crate::private_v8::data::v8__Value__SameValue(a, b) || {
            let zero = &*$crate::private_v8::Local::<$crate::private_v8::data::Value>::from($crate::private_v8::data::Integer::zero());
            $crate::private_v8::data::v8__Value__StrictEquals(a, zero) && $crate::private_v8::data::v8__Value__StrictEquals(b, zero)
          }
        }
      }
    }
  };
}
