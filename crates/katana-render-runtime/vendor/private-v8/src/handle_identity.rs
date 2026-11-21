use std::hash::{Hash, Hasher};

use super::{Global, Handle, Local};

impl<T> Eq for Local<'_, T> where T: Eq {}
impl<T> Eq for Global<T> where T: Eq {}

impl<T: Hash> Hash for Local<'_, T> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    (**self).hash(state);
  }
}

impl<T: Hash> Hash for Global<T> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    /* Hashing may call into V8 (e.g. `Object::GetIdentityHash`, which can */
    /* mutate the object), so it needs the same gate as any other access. */
    if self.isolate_liveness.get_isolate_ptr().is_null() {
      panic!("can't hash Global after its host Isolate has been disposed");
    }
    self.assert_access_allowed();
    unsafe { self.data.as_ref().hash(state) }
  }
}

impl<T, Rhs: Handle> PartialEq<Rhs> for Local<'_, T>
where
  T: PartialEq<Rhs::Data>,
{
  fn eq(&self, other: &Rhs) -> bool {
    let i1 = self.get_handle_info();
    let i2 = other.get_handle_info();
    if i1.host.are_different_live_isolates(i2.host) {
      return false;
    }
    self.assert_safe_to_access();
    other.assert_safe_to_access();
    i1.host.match_host(i2.host, None)
      && unsafe { i1.data.as_ref() == i2.data.as_ref() }
  }
}

impl<T, Rhs: Handle> PartialEq<Rhs> for Global<T>
where
  T: PartialEq<Rhs::Data>,
{
  fn eq(&self, other: &Rhs) -> bool {
    let i1 = self.get_handle_info();
    let i2 = other.get_handle_info();
    /* Distinct live isolates cannot contain the same V8 object. This check */
    /* does not touch either isolate, so preserve the historical `false` */
    /* result even if one Global is currently off its home thread. */
    if i1.host.are_different_live_isolates(i2.host) {
      return false;
    }
    self.assert_safe_to_access();
    other.assert_safe_to_access();
    if !i1.host.match_host(i2.host, None) {
      return false;
    }
    /* Comparison calls into V8 (e.g. `Value::SameValue`); both operands */
    /* were gated by `assert_safe_to_access` above. */
    unsafe { i1.data.as_ref() == i2.data.as_ref() }
  }
}
