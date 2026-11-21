use super::*;
use std::any::type_name;
use std::thread::yield_now;
use std::time::{Duration, Instant};
use std::mem::forget;
use std::ptr::null;

#[derive(Eq, PartialEq)]
struct MockSharedObj {
  pub inner: u32,
}

impl MockSharedObj {
  const INSTANCE_A: Self = Self { inner: 11111 };
  const INSTANCE_B: Self = Self { inner: 22222 };

  const SHARED_PTR_BASE_A: SharedPtrBase<Self> =
    SharedPtrBase([1, 1], PhantomData);
  const SHARED_PTR_BASE_B: SharedPtrBase<Self> =
    SharedPtrBase([2, 2], PhantomData);
}

impl Shared for MockSharedObj {
  fn clone(_: &SharedPtrBase<Self>) -> SharedPtrBase<Self> {
    unreachable!("the fixed reference-count fixture only exercises moves")
  }

  fn from_unique_ptr(_: UniquePtr<Self>) -> SharedPtrBase<Self> {
    unreachable!(
      "the fixture creates shared pointers from fixed reference counts"
    )
  }

  fn get(p: &SharedPtrBase<Self>) -> *const Self {
    match p {
      &Self::SHARED_PTR_BASE_A => &Self::INSTANCE_A,
      &Self::SHARED_PTR_BASE_B => &Self::INSTANCE_B,
      p if p == &Default::default() => null(),
      _ => unreachable!(),
    }
  }

  fn reset(p: &mut SharedPtrBase<Self>) {
    forget(take(p));
  }

  fn use_count(p: &SharedPtrBase<Self>) -> long {
    match p {
      &Self::SHARED_PTR_BASE_A => 1,
      &Self::SHARED_PTR_BASE_B => 2,
      p if p == &Default::default() => 0,
      _ => unreachable!(),
    }
  }
}

#[test]
fn shared_ptr_and_shared_ref() {
  let cases = [
    (MockSharedObj::SHARED_PTR_BASE_A, 11111, 1),
    (MockSharedObj::SHARED_PTR_BASE_B, 22222, 2),
  ];
  for (base, inner, count) in cases {
    let mut original = SharedPtr(base);
    assert!(!original.is_null());
    original.assert_use_count_eq(count);

    let reference: SharedRef<_> = original.take().unwrap();
    assert_eq!(reference.inner, inner);
    reference.assert_use_count_eq(count);
    assert!(original.is_null());
    original.assert_use_count_eq(0);

    let restored: SharedPtr<_> = reference.into();
    assert!(!restored.is_null());
    restored.assert_use_count_eq(count);
    assert_eq!(restored.unwrap().inner, inner);
  }
}

#[test]
#[should_panic(expected = "assertion failed: \
    `SharedPtr<katana_render_runtime::private_v8::support::shared::tests::MockSharedObj>` reference count \
    does not match expectation")]
fn shared_ptr_use_count_assertion_failed() {
  let shared_ptr: SharedPtr<MockSharedObj> = Default::default();
  shared_ptr.assert_use_count_eq(3);
}

#[test]
#[should_panic(expected = "assertion failed: \
    `SharedRef<katana_render_runtime::private_v8::support::shared::tests::MockSharedObj>` reference count \
    does not match expectation")]
fn shared_ref_use_count_assertion_failed() {
  let shared_ref = SharedRef(MockSharedObj::SHARED_PTR_BASE_B);
  shared_ref.assert_use_count_eq(7);
}

impl<T: Shared> SharedPtr<T> {
  #[track_caller]
  pub fn assert_use_count_eq(&self, expected: usize) {
    assert_shared_ptr_use_count_eq("SharedPtr", &self.0, expected);
  }

  pub fn take(&mut self) -> Option<SharedRef<T>> {
    if self.is_null() {
      None
    } else {
      let base = take(&mut self.0);
      Some(SharedRef(base))
    }
  }

}

impl<T: Shared> SharedRef<T> {
  #[track_caller]
  pub fn assert_use_count_eq(&self, expected: usize) {
    assert_shared_ptr_use_count_eq("SharedRef", &self.0, expected);
  }
}

#[track_caller]
fn assert_shared_ptr_use_count_eq<T: Shared>(
  wrapper_type_name: &str,
  shared_ptr: &SharedPtrBase<T>,
  expected: usize,
) {
  let mut actual = T::use_count(shared_ptr);
  let ok = match long::try_from(expected) {
    Err(_) => false, // Non-`long` value can never match actual use count.
    Ok(expected) if actual == expected => true, // Fast path.
    Ok(expected) => {
      pub const RETRY_TIMEOUT: Duration = Duration::from_secs(1);
      let start = Instant::now();
      loop {
        yield_now();
        actual = T::use_count(shared_ptr);
        if actual == expected {
          break true;
        } else if start.elapsed() > RETRY_TIMEOUT {
          break false;
        }
      }
    }
  };
  assert!(
    ok,
    "assertion failed: `{wrapper_type_name}<{}>` reference count does not match expectation\
       \n   actual: {actual}\
       \n expected: {expected}",
    type_name::<T>(),
  );
}
