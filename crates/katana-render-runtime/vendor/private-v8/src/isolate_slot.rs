use std::any::TypeId;
use std::hash::BuildHasher;
use std::hash::Hasher;
use std::mem::MaybeUninit;
use std::mem::align_of;
use std::mem::needs_drop;
use std::mem::size_of;
use std::ptr::drop_in_place;
use std::ptr;

/// A special hasher that is optimized for hashing `std::any::TypeId` values.
/// `TypeId` values are actually 64-bit values which themselves come out of some
/// hash function, so it's unnecessary to shuffle their bits any further.
#[derive(Clone, Default)]
pub(crate) struct TypeIdHasher {
  state: Option<u64>,
}

impl Hasher for TypeIdHasher {
  fn write(&mut self, _bytes: &[u8]) {
    panic!("TypeIdHasher::write() called unexpectedly");
  }

  #[inline]
  fn write_u64(&mut self, value: u64) {
    /* The internal hash function of TypeId only takes the bottom 64-bits, even on versions */
    /* of Rust that use a 128-bit TypeId. */
    let prev_state = self.state.replace(value);
    debug_assert_eq!(prev_state, None);
  }

  #[inline]
  fn finish(&self) -> u64 {
    self.state.unwrap()
  }
}

/// Factory for instances of `TypeIdHasher`. This is the type that one would
/// pass to the constructor of some map/set type in order to make it use
/// `TypeIdHasher` instead of the default hasher implementation.
#[derive(Copy, Clone, Default)]
pub(crate) struct BuildTypeIdHasher;

impl BuildHasher for BuildTypeIdHasher {
  type Hasher = TypeIdHasher;

  #[inline]
  fn build_hasher(&self) -> Self::Hasher {
    Default::default()
  }
}

const _: () = {
  assert!(
    size_of::<TypeId>() == size_of::<u64>()
      || size_of::<TypeId>() == size_of::<u128>()
  );
  assert!(
    align_of::<TypeId>() == align_of::<u64>()
      || align_of::<TypeId>() == align_of::<u128>()
  );
};

pub(crate) struct RawSlot {
  data: RawSlotData,
  dtor: Option<RawSlotDtor>,
}

type RawSlotData = MaybeUninit<usize>;
type RawSlotDtor = unsafe fn(&mut RawSlotData) -> ();

impl RawSlot {
  #[inline]
  pub fn new<T: 'static>(value: T) -> Self {
    if Self::needs_box::<T>() {
      Self::new_internal(Box::new(value))
    } else {
      Self::new_internal(value)
    }
  }

  /* SAFETY: a valid value of type `T` must haven been stored in the slot */
  /* earlier. There is no verification that the type param provided by the */
  /* caller is correct. */
  #[inline]
  pub unsafe fn borrow<T: 'static>(&self) -> &T {
    unsafe {
      if Self::needs_box::<T>() {
        &*(self.data.as_ptr() as *const Box<T>)
      } else {
        &*(self.data.as_ptr() as *const T)
      }
    }
  }

  const fn needs_box<T: 'static>() -> bool {
    size_of::<T>() > size_of::<RawSlotData>()
      || align_of::<T>() > align_of::<RawSlotData>()
  }

  #[inline]
  fn new_internal<B: 'static>(value: B) -> Self {
    assert!(!Self::needs_box::<B>());
    let mut self_ = Self {
      data: RawSlotData::zeroed(),
      dtor: None,
    };
    unsafe {
      ptr::write(self_.data.as_mut_ptr() as *mut B, value);
    }
    if needs_drop::<B>() {
      self_.dtor.replace(Self::drop_internal::<B>);
    };
    self_
  }

  /* SAFETY: a valid value of type `T` or `Box<T>` must be stored in the slot. */
  unsafe fn drop_internal<B: 'static>(data: &mut RawSlotData) {
    assert!(!Self::needs_box::<B>());
    unsafe {
      drop_in_place(data.as_mut_ptr() as *mut B);
    }
  }
}

impl Drop for RawSlot {
  fn drop(&mut self) {
    if let Some(dtor) = self.dtor {
      unsafe { dtor(&mut self.data) };
    }
  }
}
