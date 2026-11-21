use std::borrow::Borrow;
use std::marker::PhantomData;
use std::mem::take;
use std::ops::Deref;

#[cfg(test)]
use super::long;
use super::{UniquePtr, UniqueRef};

pub trait Shared
where
  Self: Sized,
{
  fn clone(shared_ptr: &SharedPtrBase<Self>) -> SharedPtrBase<Self>;
  fn from_unique_ptr(unique_ptr: UniquePtr<Self>) -> SharedPtrBase<Self>;
  fn get(shared_ptr: &SharedPtrBase<Self>) -> *const Self;
  fn reset(shared_ptr: &mut SharedPtrBase<Self>);
  #[cfg(test)]
  fn use_count(shared_ptr: &SharedPtrBase<Self>) -> long;
}

/// Private base type which is shared by the `SharedPtr` and `SharedRef`
/// implementations.
#[repr(C)]
#[derive(Eq, Debug, PartialEq)]
pub struct SharedPtrBase<T: Shared>([usize; 2], PhantomData<T>);

unsafe impl<T: Shared + Sync> Send for SharedPtrBase<T> {}
unsafe impl<T: Shared + Sync> Sync for SharedPtrBase<T> {}

impl<T: Shared> Default for SharedPtrBase<T> {
  fn default() -> Self {
    Self([0usize; 2], PhantomData)
  }
}

impl<T: Shared> Drop for SharedPtrBase<T> {
  fn drop(&mut self) {
    <T as Shared>::reset(self);
  }
}

/// Wrapper around a C++ shared_ptr. A shared_ptr may be be null.
#[repr(C)]
#[derive(Debug)]
pub struct SharedPtr<T: Shared>(SharedPtrBase<T>);

impl<T: Shared> SharedPtr<T> {
  pub fn is_null(&self) -> bool {
    <T as Shared>::get(&self.0).is_null()
  }

  pub fn unwrap(self) -> SharedRef<T> {
    assert!(!self.is_null());
    SharedRef(self.0)
  }
}

impl<T: Shared> Clone for SharedPtr<T> {
  fn clone(&self) -> Self {
    Self(<T as Shared>::clone(&self.0))
  }
}

impl<T: Shared> Default for SharedPtr<T> {
  fn default() -> Self {
    Self(Default::default())
  }
}

impl<T, U> From<U> for SharedPtr<T>
where
  T: Shared,
  U: Into<UniquePtr<T>>,
{
  fn from(unique_ptr: U) -> Self {
    let unique_ptr = unique_ptr.into();
    Self(<T as Shared>::from_unique_ptr(unique_ptr))
  }
}

impl<T: Shared> From<SharedRef<T>> for SharedPtr<T> {
  fn from(mut shared_ref: SharedRef<T>) -> Self {
    Self(take(&mut shared_ref.0))
  }
}

/// Wrapper around a C++ shared_ptr. The shared_ptr is assumed to contain a
/// value and may not be null.
#[repr(C)]
#[derive(Debug)]
pub struct SharedRef<T: Shared>(SharedPtrBase<T>);

impl<T: Shared> Clone for SharedRef<T> {
  fn clone(&self) -> Self {
    Self(<T as Shared>::clone(&self.0))
  }
}

impl<T: Shared> From<UniqueRef<T>> for SharedRef<T> {
  fn from(unique_ref: UniqueRef<T>) -> Self {
    SharedPtr::from(unique_ref).unwrap()
  }
}

impl<T: Shared> Deref for SharedRef<T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    unsafe { &*(<T as Shared>::get(&self.0)) }
  }
}

impl<T: Shared> AsRef<T> for SharedRef<T> {
  fn as_ref(&self) -> &T {
    self
  }
}

impl<T: Shared> Borrow<T> for SharedRef<T> {
  fn borrow(&self) -> &T {
    self
  }
}

#[cfg(test)]
#[path = "support_shared_tests.rs"]
mod tests;
