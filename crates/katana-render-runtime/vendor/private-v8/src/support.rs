use std::marker::PhantomData;

#[path = "support_callback.rs"]
mod callback;
#[path = "support_shared.rs"]
mod shared;
#[path = "support_unique.rs"]
mod unique;

pub use callback::{MapFnFrom, MapFnTo, ToCFn, UnitType};
pub use shared::{Shared, SharedPtr, SharedPtrBase, SharedRef};
pub use unique::{UniquePtr, UniqueRef};

/* TODO use libc::intptr_t when stable. */
/* https://doc.rust-lang.org/1.7.0/libc/type.intptr_t.html */
#[allow(non_camel_case_types)]
pub type intptr_t = isize;

pub use std::os::raw::c_char as char;
pub use std::os::raw::c_int as int;
#[cfg(test)]
pub use std::os::raw::c_long as long;

pub type Opaque = [u8; 0];

#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub enum MaybeBool {
  JustFalse = 0,
  JustTrue = 1,
  Nothing = 2,
}

impl From<MaybeBool> for Option<bool> {
  fn from(b: MaybeBool) -> Self {
    match b {
      MaybeBool::JustFalse => Some(false),
      MaybeBool::JustTrue => Some(true),
      MaybeBool::Nothing => None,
    }
  }
}

impl From<Option<bool>> for MaybeBool {
  fn from(option: Option<bool>) -> Self {
    match option {
      Some(false) => MaybeBool::JustFalse,
      Some(true) => MaybeBool::JustTrue,
      None => MaybeBool::Nothing,
    }
  }
}

#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct CxxVTable(pub *const Opaque);

#[allow(unused)]
#[derive(Copy, Clone, Debug)]
pub struct RustVTable<DynT>(pub *const Opaque, pub PhantomData<DynT>);

#[derive(Debug)]
pub struct FieldOffset<F>(usize, PhantomData<F>);

unsafe impl<F> Send for FieldOffset<F> where F: Send {}
unsafe impl<F> Sync for FieldOffset<F> where F: Sync {}

impl<F> Copy for FieldOffset<F> {}

impl<F> Clone for FieldOffset<F> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<F> FieldOffset<F> {
  pub fn from_ptrs<E>(embedder_ptr: *const E, field_ptr: *const F) -> Self {
    let embedder_addr = embedder_ptr as usize;
    let field_addr = field_ptr as usize;
    assert!(field_addr >= embedder_addr);
    assert!((field_addr + size_of::<F>()) <= (embedder_addr + size_of::<E>()));
    Self(field_addr - embedder_addr, PhantomData)
  }

  #[allow(clippy::wrong_self_convention)]
  pub unsafe fn to_embedder<E>(self, field: &F) -> &E {
    unsafe {
      (((field as *const _ as usize) - self.0) as *const E)
        .as_ref()
        .unwrap()
    }
  }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Maybe<T> {
  has_value: bool,
  value: T,
}

impl<T> From<Maybe<T>> for Option<T> {
  fn from(maybe: Maybe<T>) -> Self {
    if maybe.has_value {
      Some(maybe.value)
    } else {
      None
    }
  }
}

macro_rules! assert_layout_subset {
  ($subset: ty, $superset: ty { $($field: ident),* $(,)? }) => {
    const _: () = {
      if std::mem::size_of::<$subset>() > std::mem::size_of::<$superset>() {
        panic!(concat!(
          "assertion failed: ",
          "size of `",
          stringify!($subset),
          "` is greater than `",
          stringify!($superset),
          "`"
        ));
      }
      if !(std::mem::align_of::<$subset>() == std::mem::align_of::<$superset>()) {
        panic!(concat!(
          "assertion failed: `",
          stringify!($subset),
          "` and `",
          stringify!($superset),
          "` have different alignments"
        ));
      }
      $(
        if std::mem::offset_of!($subset, $field) != std::mem::offset_of!($superset, $field) {
          panic!(concat!(
            "assertion failed: `",
            stringify!($subset),
            "` and `",
            stringify!($superset),
            "` have different offsets for field `",
            stringify!($field),
            "`"
          ));
        }
      )*
    };
  };
}

pub(crate) use assert_layout_subset;
