use super::*;
unsafe extern "C" {
  #[cfg(test)]
  pub(super) fn v8_inspector__V8Inspector__Channel__BASE__CONSTRUCT(
    buf: *mut MaybeUninit<RawChannel>,
  );

  pub(super) fn v8_inspector__StringBuffer__DELETE(this: *mut StringBuffer);
  #[cfg(test)]
  pub(super) fn v8_inspector__StringBuffer__string(this: &StringBuffer) -> StringView<'_>;
  #[cfg(test)]
  pub(super) fn v8_inspector__StringBuffer__create(
    source: StringView,
  ) -> UniquePtr<StringBuffer>;

  pub(super) fn v8_inspector__V8StackTrace__DELETE(this: *mut V8StackTrace);
}
