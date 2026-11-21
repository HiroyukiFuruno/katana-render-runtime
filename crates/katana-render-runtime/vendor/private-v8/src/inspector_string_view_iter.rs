use super::*;
use super::inspector_string_view::StringView;

impl<'a> IntoIterator for StringView<'a> {
  type IntoIter = StringViewIterator<'a>;
  type Item = u16;

  fn into_iter(self) -> Self::IntoIter {
    StringViewIterator { view: self, pos: 0 }
  }
}

#[derive(Copy, Clone, Debug)]
pub struct StringViewIterator<'a> {
  view: StringView<'a>,
  pos: usize,
}

impl Iterator for StringViewIterator<'_> {
  type Item = u16;

  fn next(&mut self) -> Option<Self::Item> {
    let result = Some(match self.view {
      StringView::U16(v) => v.get_at(self.pos)?,
      StringView::U8(v) => u16::from(v.get_at(self.pos)?),
    });
    self.pos += 1;
    result
  }
}

impl ExactSizeIterator for StringViewIterator<'_> {
  fn len(&self) -> usize {
    self.view.len()
  }
}
