use super::*;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;
use std::slice;
use std::string;

/* Notes: */
/*  * This class is ported, not wrapped using bindings. */
/*  * Since Rust `repr(bool)` is not allowed, we're assuming that `bool` and */
/*    `u8` have the same size. This is assumption is checked in 'support.h'. */
/*    TODO: find/open upstream issue to allow #[repr(bool)] support. */

#[derive(Clone, Debug, Copy)]
#[repr(u8)]
pub enum StringView<'a> {
  /* Do not reorder! */
  U16(CharacterArray<'a, u16>),
  U8(CharacterArray<'a, u8>),
}

impl<'a> From<&'a [u8]> for StringView<'a> {
  fn from(v: &'a [u8]) -> Self {
    Self::U8(CharacterArray::<'a, u8>::from(v))
  }
}

impl<'a> From<&'a [u16]> for StringView<'a> {
  fn from(v: &'a [u16]) -> Self {
    Self::U16(CharacterArray::<'a, u16>::from(v))
  }
}

impl StringView<'_> {
  pub fn len(&self) -> usize {
    match self {
      Self::U16(v) => v.len(),
      Self::U8(v) => v.len(),
    }
  }


}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CharacterArray<'a, T> {
  m_length: usize,
  m_characters: *const T,
  _phantom: PhantomData<&'a T>,
}

impl<T> CharacterArray<'_, T>
where
  T: Copy,
{
  #[inline(always)]
  fn len(&self) -> usize {
    self.m_length
  }

  #[inline(always)]
  pub(super) fn get_at(&self, index: usize) -> Option<T> {
    if index < self.m_length {
      Some(unsafe { *self.m_characters.add(index) })
    } else {
      None
    }
  }
}

unsafe impl<T> Send for CharacterArray<'_, T> where T: Copy {}
unsafe impl<T> Sync for CharacterArray<'_, T> where T: Sync {}

impl fmt::Display for CharacterArray<'_, u8> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str(
      self
        .iter()
        .cloned()
        .map(char::from)
        .collect::<string::String>()
        .as_str(),
    )
  }
}

impl fmt::Display for CharacterArray<'_, u16> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str(&string::String::from_utf16_lossy(self))
  }
}

impl<'a, T> From<&'a [T]> for CharacterArray<'a, T> {
  fn from(v: &'a [T]) -> Self {
    Self {
      m_length: v.len(),
      m_characters: v.as_ptr(),
      _phantom: PhantomData,
    }
  }
}

impl<T> Deref for CharacterArray<'_, T> {
  type Target = [T];

  fn deref(&self) -> &[T] {
    let Self {
      m_length,
      mut m_characters,
      ..
    } = *self;
    if m_characters.is_null() {
      assert_eq!(m_length, 0);
      m_characters = NonNull::dangling().as_ptr();
    };
    unsafe { slice::from_raw_parts(m_characters, m_length) }
  }
}

#[test]
fn string_view_display() {
  let ok: [u16; 2] = [111, 107];
  assert_eq!("ok", format!("{}", StringView::from(&ok[..])));
  assert_eq!("ok", format!("{}", StringView::from(&b"ok"[..])));
  assert_eq!("ØÞ", format!("{}", StringView::from(&[216u8, 222u8][..])));
}
