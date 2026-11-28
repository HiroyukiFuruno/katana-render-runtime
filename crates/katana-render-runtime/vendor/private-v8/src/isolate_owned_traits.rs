use super::Isolate;
use super::isolate_owned_type::OwnedIsolate;

impl AsMut<Isolate> for OwnedIsolate {
  fn as_mut(&mut self) -> &mut Isolate {
    self
  }
}

impl AsMut<Isolate> for Isolate {
  fn as_mut(&mut self) -> &mut Isolate {
    self
  }
}

impl AsRef<Isolate> for OwnedIsolate {
  fn as_ref(&self) -> &Isolate {
    unsafe { Isolate::from_raw_ref(&self.cxx_isolate) }
  }
}

impl AsRef<Isolate> for Isolate {
  fn as_ref(&self) -> &Isolate {
    self
  }
}
