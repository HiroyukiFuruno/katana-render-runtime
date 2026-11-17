use std::borrow::{Borrow, BorrowMut};
use std::mem::{align_of, forget, size_of, take};
use std::ops::{Deref, DerefMut};
use std::ptr::{NonNull, drop_in_place};

use super::{Shared, SharedRef};

/// Pointer to object allocated on the C++ heap. The pointer may be null.
#[repr(transparent)]
#[derive(Debug)]
pub struct UniquePtr<T: ?Sized>(Option<UniqueRef<T>>);

impl<T: ?Sized> UniquePtr<T> {
  #[cfg(test)]
  pub fn as_mut(&mut self) -> Option<&mut UniqueRef<T>> {
    self.0.as_mut()
  }

  pub fn take(&mut self) -> Option<UniqueRef<T>> {
    take(&mut self.0)
  }
}

impl<T> Default for UniquePtr<T> {
  fn default() -> Self {
    assert_unique_ptr_layout_compatible::<Self, T>();
    Self(None)
  }
}

impl<T> From<UniqueRef<T>> for UniquePtr<T> {
  fn from(unique_ref: UniqueRef<T>) -> Self {
    assert_unique_ptr_layout_compatible::<Self, T>();
    Self(Some(unique_ref))
  }
}

/// Pointer to object allocated on the C++ heap. The pointer may not be null.
#[repr(transparent)]
#[derive(Debug)]
pub struct UniqueRef<T: ?Sized>(NonNull<T>);

impl<T> UniqueRef<T> {
  pub(crate) unsafe fn try_from_raw(ptr: *mut T) -> Option<Self> {
    assert_unique_ptr_layout_compatible::<Self, T>();
    NonNull::new(ptr).map(Self)
  }

  pub(crate) unsafe fn from_raw(ptr: *mut T) -> Self {
    assert_unique_ptr_layout_compatible::<Self, T>();
    unsafe { Self::try_from_raw(ptr).unwrap() }
  }

  pub fn into_raw(self) -> *mut T {
    let ptr = self.0.as_ptr();
    forget(self);
    ptr
  }

}

impl<T: Shared> UniqueRef<T> {
  pub fn make_shared(self) -> SharedRef<T> {
    self.into()
  }
}

impl<T: ?Sized> Drop for UniqueRef<T> {
  fn drop(&mut self) {
    unsafe { drop_in_place(self.0.as_ptr()) }
  }
}

impl<T: ?Sized> Deref for UniqueRef<T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    unsafe { self.0.as_ref() }
  }
}

impl<T: ?Sized> DerefMut for UniqueRef<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { self.0.as_mut() }
  }
}

impl<T: ?Sized> AsRef<T> for UniqueRef<T> {
  fn as_ref(&self) -> &T {
    self
  }
}

impl<T: ?Sized> AsMut<T> for UniqueRef<T> {
  fn as_mut(&mut self) -> &mut T {
    self
  }
}

impl<T: ?Sized> Borrow<T> for UniqueRef<T> {
  fn borrow(&self) -> &T {
    self
  }
}

impl<T: ?Sized> BorrowMut<T> for UniqueRef<T> {
  fn borrow_mut(&mut self) -> &mut T {
    self
  }
}

fn assert_unique_ptr_layout_compatible<U, T>() {
  /* Assert that `U` (a `UniqueRef` or `UniquePtr`) has the same memory layout */
  /* as a raw C pointer. */
  assert_eq!(size_of::<U>(), size_of::<*mut T>());
  assert_eq!(align_of::<U>(), align_of::<*mut T>());
}
