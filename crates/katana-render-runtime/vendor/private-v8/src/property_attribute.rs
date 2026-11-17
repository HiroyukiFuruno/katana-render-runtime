#[repr(C)]
#[derive(Debug, Eq, PartialEq)]
pub struct PropertyAttribute(u32);

impl PropertyAttribute {
  /// No property attributes.
  pub const NONE: Self = Self(0);

  /// Not writable. Corresponds to
  /// `Object.defineProperty(o, "p", { writable: false })`.
  #[cfg(test)]
  pub const READ_ONLY: Self = Self(1 << 0);

  /// Not enumerable. Corresponds to
  /// `Object.defineProperty(o, "p", { enumerable: false })`.
  #[cfg(test)]
  pub const DONT_ENUM: Self = Self(1 << 1);

  /// Not configurable. Corresponds to
  /// `Object.defineProperty(o, "p", { configurable: false })`.
  #[cfg(test)]
  pub const DONT_DELETE: Self = Self(1 << 2);

  /// Test if no property attributes are set.
  #[cfg(test)]
  #[inline(always)]
  pub fn is_none(&self) -> bool {
    *self == PropertyAttribute::NONE
  }

  /// Test if the read-only property attribute is set.
  #[cfg(test)]
  #[inline(always)]
  pub fn is_read_only(&self) -> bool {
    self.has(Self::READ_ONLY)
  }

  /// Test if the non-enumerable property attribute is set.
  #[cfg(test)]
  #[inline(always)]
  pub fn is_dont_enum(&self) -> bool {
    self.has(Self::DONT_ENUM)
  }

  /// Test if the non-configurable property attribute is set.
  #[cfg(test)]
  #[inline(always)]
  pub fn is_dont_delete(&self) -> bool {
    self.has(Self::DONT_DELETE)
  }

  #[cfg(test)]
  #[inline(always)]
  fn has(&self, that: Self) -> bool {
    let Self(lhs) = self;
    let Self(rhs) = that;
    0 != lhs & rhs
  }

}

/* Identical to #[derive(Default)] but arguably clearer when made explicit. */
#[cfg(test)]
impl Default for PropertyAttribute {
  fn default() -> Self {
    Self::NONE
  }
}

#[cfg(test)]
impl std::ops::BitOr for PropertyAttribute {
  type Output = Self;

  fn bitor(self, Self(rhs): Self) -> Self {
    let Self(lhs) = self;
    Self(lhs | rhs)
  }
}

#[test]
fn test_attr() {
  let cases = [
    (PropertyAttribute::NONE, [true, false, false, false]),
    (PropertyAttribute::READ_ONLY, [false, true, false, false]),
    (PropertyAttribute::DONT_ENUM, [false, false, true, false]),
    (PropertyAttribute::DONT_DELETE, [false, false, false, true]),
  ];
  for (value, expected) in cases {
    assert_eq!(
      [
        value.is_none(),
        value.is_read_only(),
        value.is_dont_enum(),
        value.is_dont_delete(),
      ],
      expected,
      "{value:?}"
    );
  }
}

#[test]
#[allow(clippy::cognitive_complexity)]
fn combined_flags_preserve_their_properties() {
  assert_eq!(PropertyAttribute::NONE, Default::default());
  assert_eq!(
    PropertyAttribute::READ_ONLY,
    PropertyAttribute::NONE | PropertyAttribute::READ_ONLY
  );

  let attr = PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_ENUM;
  assert!(!attr.is_none());
  assert!(attr.is_read_only());
  assert!(attr.is_dont_enum());
  assert!(!attr.is_dont_delete());

  let attr = PropertyAttribute::READ_ONLY | PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_ENUM;
  assert!(!attr.is_none());
  assert!(attr.is_read_only());
  assert!(attr.is_dont_enum());
  assert!(!attr.is_dont_delete());
}
