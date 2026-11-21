use std::sync::atomic::{AtomicUsize, Ordering};

/* Zero is reserved for a thread whose ID has not been requested yet. IDs are */
/* never reused; abort before wrapping rather than risk confusing two threads. */
static NEXT_THREAD_ID: AtomicUsize = AtomicUsize::new(1);

thread_local! {
  /* Const-initialized and destructor-free, so this remains readable while */
  /* other TLS values are being destroyed. `Global::clone`/`drop` hit this on */
  /* every call. */
  static CURRENT_THREAD_ID: std::cell::Cell<usize> =
    const { std::cell::Cell::new(0) };
}

#[inline(always)]
pub(super) fn current_thread_id() -> usize {
  CURRENT_THREAD_ID.with(|current| {
    let id = current.get();
    if id != 0 {
      return id;
    }

    /* This cold path deliberately avoids `std::thread::current()`: it may run */
    /* from another TLS destructor, where panicking would abort the process. */
    /* A relaxed global counter is sufficient because IDs are only compared. */
    let id = NEXT_THREAD_ID
      .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
        next.checked_add(1)
      })
      .unwrap_or_else(|_| std::process::abort());
    current.set(id);
    id
  })
}
