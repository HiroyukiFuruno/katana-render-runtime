use std::any::type_name;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug)]
pub enum DataError {
  BadType {
    actual: &'static str,
    expected: &'static str,
  },
  NoData {
    expected: &'static str,
  },
}

impl DataError {
  pub(crate) fn bad_type<E: 'static, A: 'static>() -> Self {
    Self::BadType {
      expected: type_name::<E>(),
      actual: type_name::<A>(),
    }
  }

  #[allow(dead_code)]
  pub(crate) fn no_data<E: 'static>() -> Self {
    Self::NoData {
      expected: type_name::<E>(),
    }
  }
}

impl Display for DataError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Self::BadType { expected, actual } => {
        write!(f, "expected type `{expected}`, got `{actual}`")
      }
      Self::NoData { expected } => {
        write!(f, "expected `Some({expected})`, found `None`")
      }
    }
  }
}

impl Error for DataError {}
