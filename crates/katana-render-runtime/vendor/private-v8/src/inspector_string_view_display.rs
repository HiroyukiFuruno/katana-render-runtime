use super::*;
use super::inspector_string_view::StringView;

impl fmt::Display for StringView<'_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Self::U16(v) => write!(f, "{v}"),
      Self::U8(v) => write!(f, "{v}"),
    }
  }
}
