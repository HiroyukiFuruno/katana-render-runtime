use super::ACTIVE_SCOPE;
#[cfg(all(test, target_os = "macos"))]
use super::observation;
use std::{marker::PhantomData, rc::Rc};

pub(crate) struct FontFaceCacheScope(PhantomData<Rc<()>>);

impl FontFaceCacheScope {
    pub(crate) fn enter() -> Self {
        ACTIVE_SCOPE.with(|scope| scope.borrow_mut().enter());
        #[cfg(all(test, target_os = "macos"))]
        observation::publish();
        Self(PhantomData)
    }
}

impl Drop for FontFaceCacheScope {
    fn drop(&mut self) {
        ACTIVE_SCOPE.with(|scope| scope.borrow_mut().leave());
        #[cfg(all(test, target_os = "macos"))]
        observation::publish();
    }
}
