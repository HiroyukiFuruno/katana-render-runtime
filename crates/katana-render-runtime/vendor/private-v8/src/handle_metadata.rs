use std::ptr::NonNull;

use crate::private_v8::{Isolate, IsolateHandle, isolate::RealIsolate};

#[derive(Copy, Debug, Clone)]
pub struct HandleInfo<T> {
  pub(super) data: NonNull<T>,
  pub(super) host: HandleHost,
}

impl<T> HandleInfo<T> {
  pub(super) fn new(data: NonNull<T>, host: HandleHost) -> Self {
    Self { data, host }
  }
}

#[derive(Copy, Debug, Clone)]
pub(super) enum HandleHost {
  /* Note: the `HandleHost::Scope` variant does not indicate that the handle */
  /* it applies to is not associated with an `Isolate`. It only means that */
  /* the handle is a `Local` handle that was unable to provide a pointer to */
  /* the `Isolate` that hosts it (the handle) and the currently entered */
  /* scope. */
  Scope,
  Isolate(NonNull<RealIsolate>),
  DisposedIsolate,
}

impl From<&'_ Isolate> for HandleHost {
  fn from(isolate: &'_ Isolate) -> Self {
    Self::Isolate(unsafe { NonNull::new_unchecked(isolate.as_real_ptr()) })
  }
}

impl From<&'_ IsolateHandle> for HandleHost {
  fn from(isolate_handle: &IsolateHandle) -> Self {
    NonNull::new(unsafe { isolate_handle.get_isolate_ptr() })
      .map_or(Self::DisposedIsolate, Self::Isolate)
  }
}

impl HandleHost {
  pub(super) fn are_different_live_isolates(self, other: Self) -> bool {
    matches!(
      (self, other),
      (Self::Isolate(left), Self::Isolate(right)) if left != right
    )
  }

  /// Compares two `HandleHost` values, returning `true` if they refer to the
  /// same `Isolate`, or `false` if they refer to different isolates.
  ///
  /// If the caller knows which `Isolate` the currently entered scope (if any)
  /// belongs to, it should pass on this information via the second argument
  /// (`scope_isolate_opt`).
  ///
  /// # Panics
  ///
  /// This function panics if one of the `HandleHost` values refers to an
  /// `Isolate` that has been disposed.
  ///
  /// # Safety / Bugs
  ///
  /// The current implementation is a bit too forgiving. If it cannot decide
  /// whether two hosts refer to the same `Isolate`, it just returns `true`.
  /// Note that this can only happen when the caller does _not_ provide a value
  /// for the `scope_isolate_opt` argument.
  pub(super) fn match_host(
    self,
    other: Self,
    scope_isolate_opt: Option<&Isolate>,
  ) -> bool {
    let scope_isolate_opt_nn = scope_isolate_opt
      .map(|isolate| unsafe { NonNull::new_unchecked(isolate.as_real_ptr()) });
    match (self, other, scope_isolate_opt_nn) {
      (Self::Scope, Self::Scope, _) => true,
      (Self::Isolate(ile1), Self::Isolate(ile2), _) => ile1 == ile2,
      (Self::Scope, Self::Isolate(ile1), Some(ile2)) => ile1 == ile2,
      (Self::Isolate(ile1), Self::Scope, Some(ile2)) => ile1 == ile2,
      /* TODO(pisciaureus): If the caller didn't provide a `scope_isolate_opt` */
      /* value that works, we can't do a meaningful check. So all we do for now */
      /* is pretend the Isolates match and hope for the best. This eventually */
      /* needs to be tightened up. */
      (Self::Scope, Self::Isolate(_), _) => true,
      (Self::Isolate(_), Self::Scope, _) => true,
      /* Handles hosted in an Isolate that has been disposed aren't good for */
      /* anything, even if a pair of handles used to to be hosted in the same */
      /* now-disposed solate. */
      (Self::DisposedIsolate, ..) | (_, Self::DisposedIsolate, _) => {
        panic!("attempt to access Handle hosted by disposed Isolate")
      }
    }
  }

  fn assert_match_host(self, other: Self, scope_opt: Option<&Isolate>) {
    assert!(
      self.match_host(other, scope_opt),
      "attempt to use Handle in an Isolate that is not its host"
    );
  }

  #[allow(dead_code)]
  fn match_isolate(self, isolate: &Isolate) -> bool {
    self.match_host(isolate.into(), Some(isolate))
  }

  pub(super) fn assert_match_isolate(self, isolate: &Isolate) {
    self.assert_match_host(isolate.into(), Some(isolate));
  }

  pub(super) fn get_isolate(self) -> NonNull<RealIsolate> {
    match self {
      Self::Scope => panic!("host Isolate for Handle not available"),
      Self::Isolate(ile) => ile,
      Self::DisposedIsolate => panic!("attempt to access disposed Isolate"),
    }
  }

  #[allow(dead_code)]
  pub(super) fn get_isolate_handle(self) -> IsolateHandle {
    let isolate = unsafe { Isolate::from_non_null(self.get_isolate()) };
    isolate.thread_safe_handle()
  }
}
