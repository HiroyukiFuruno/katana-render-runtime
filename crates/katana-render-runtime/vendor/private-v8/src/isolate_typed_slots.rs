use super::Isolate;

impl Isolate {
  /// Get a reference to embedder data added with `set_slot()`.
  #[inline(always)]
  pub fn get_slot<T: 'static>(&self) -> Option<&T> {
    self.get_annex().get_slot::<T>()
  }

  /// Associates state with the isolate.
  ///
  /// This method gives ownership of value to the Isolate. Exactly one object of
  /// each type can be associated with an Isolate. If called more than once with
  /// an object of the same type, the earlier version will be dropped and
  /// replaced.
  ///
  /// Returns true if value was set without replacing an existing value.
  ///
  /// The value will be dropped when the isolate is dropped.
  #[inline(always)]
  pub fn set_slot<T: 'static>(&mut self, value: T) -> bool {
    self.get_annex_mut().set_slot(value)
  }


}
