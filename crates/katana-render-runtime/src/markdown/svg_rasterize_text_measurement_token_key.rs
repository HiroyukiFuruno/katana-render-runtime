use super::{HtmlTextMeasurementToken, Proof};
use std::{
    fmt,
    hash::{Hash, Hasher},
    rc::{Rc, Weak as RcWeak},
};

#[derive(Clone)]
pub(crate) struct HtmlTextMeasurementTokenKey {
    proof: RcWeak<Proof>,
}

impl PartialEq for HtmlTextMeasurementTokenKey {
    fn eq(&self, other: &Self) -> bool {
        RcWeak::ptr_eq(&self.proof, &other.proof)
    }
}

impl Eq for HtmlTextMeasurementTokenKey {}

impl fmt::Debug for HtmlTextMeasurementTokenKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HtmlTextMeasurementTokenKey")
            .field(&self.proof.as_ptr())
            .finish()
    }
}

impl Hash for HtmlTextMeasurementTokenKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.proof.as_ptr().hash(state);
    }
}

impl HtmlTextMeasurementToken {
    pub(crate) fn key(&self) -> HtmlTextMeasurementTokenKey {
        HtmlTextMeasurementTokenKey {
            proof: Rc::downgrade(&self.proof),
        }
    }
}
