use super::{
  CallbackScope,
  Cell,
  Context,
  FunctionCallbackInfo,
  GetIsolate,
  HandleScope,
  Isolate,
  Local,
  Message,
  NonNull,
  Object,
  OwnedIsolate,
  PhantomData,
  PhantomPinned,
  Pin,
  PromiseRejectMessage,
  Scope,
  ScopeInit,
  ScopeStorage,
  raw,
  sealed,
};

impl<'s> CallbackScope<'s> {
  #[allow(clippy::new_ret_no_self)]
  pub unsafe fn new<P: NewCallbackScope<'s>>(
    param: P,
  ) -> ScopeStorage<P::NewScope> {
    ScopeStorage::new(P::make_new_scope(param))
  }
}

impl<C> AsRef<Isolate> for CallbackScope<'_, C> {
  fn as_ref(&self) -> &Isolate {
    unsafe { Isolate::from_raw_ref(&self.isolate) }
  }
}

impl<C> ScopeInit for CallbackScope<'_, C> {
  fn init_stack(storage: Pin<&mut ScopeStorage<Self>>) -> Pin<&mut Self> {
    let storage_mut = unsafe { storage.get_unchecked_mut() };
    let projected = &mut storage_mut.scope;
    unsafe { Pin::new_unchecked(projected) }
  }

  unsafe fn deinit(_scope: &mut Self) {}
}

impl<C> sealed::Sealed for CallbackScope<'_, C> {}
impl<C> Scope for CallbackScope<'_, C> {}

pub trait NewCallbackScope<'s>: Sized + GetIsolate {
  type NewScope: Scope;
  fn make_new_scope(me: Self) -> Self::NewScope;
}

fn make_new_callback_scope<'a, C>(
  isolate: impl GetIsolate,
  context: Option<NonNull<Context>>,
) -> CallbackScope<'a, C> {
  let isolate_ptr =
    unsafe { NonNull::new_unchecked(isolate.get_isolate_ptr()) };
  let isolate_ref = unsafe { Isolate::from_raw_ref(&isolate_ptr) };
  CallbackScope {
    raw_handle_scope: unsafe { raw::HandleScope::uninit() },
    isolate: isolate_ptr,
    annex: isolate_ref.get_annex_ptr(),
    context: Cell::new(context),
    _phantom: PhantomData,
    _pinned: PhantomPinned,
  }
}

impl<'s> NewCallbackScope<'s> for &'s mut Isolate {
  type NewScope = CallbackScope<'s, ()>;

  fn make_new_scope(me: Self) -> Self::NewScope {
    make_new_callback_scope(&*me, None)
  }
}

impl<'s> NewCallbackScope<'s> for &'s mut OwnedIsolate {
  type NewScope = CallbackScope<'s, ()>;

  fn make_new_scope(me: Self) -> Self::NewScope {
    make_new_callback_scope(&*me, None)
  }
}

impl<'s> NewCallbackScope<'s> for &'s FunctionCallbackInfo {
  type NewScope = CallbackScope<'s>;

  fn make_new_scope(me: Self) -> Self::NewScope {
    make_new_callback_scope(me, None)
  }
}

impl<'s> NewCallbackScope<'s> for Local<'s, Context> {
  type NewScope = CallbackScope<'s>;

  fn make_new_scope(me: Self) -> Self::NewScope {
    make_new_callback_scope(me, Some(me.as_non_null()))
  }
}

impl<'s> NewCallbackScope<'s> for Local<'s, Message> {
  type NewScope = CallbackScope<'s>;

  fn make_new_scope(me: Self) -> Self::NewScope {
    make_new_callback_scope(me, None)
  }
}

impl<'s, T: Into<Local<'s, Object>> + GetIsolate> NewCallbackScope<'s> for T {
  type NewScope = CallbackScope<'s>;

  fn make_new_scope(me: Self) -> Self::NewScope {
    make_new_callback_scope(me, None)
  }
}

impl<'s> NewCallbackScope<'s> for &'s PromiseRejectMessage<'s> {
  type NewScope = CallbackScope<'s>;

  fn make_new_scope(me: Self) -> Self::NewScope {
    make_new_callback_scope(me, None)
  }
}

impl<'s> AsRef<Pin<&'s mut HandleScope<'s, ()>>> for CallbackScope<'s, ()> {
  fn as_ref(&self) -> &Pin<&'s mut HandleScope<'s, ()>> {
    unsafe { std::mem::transmute(self) }
  }
}
