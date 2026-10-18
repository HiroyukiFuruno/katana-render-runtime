use super::super::proof::CreationProof;
#[cfg(test)]
use resvg::usvg::fontdb::Source;
use resvg::usvg::{
    self,
    fontdb::{Database, ID},
};
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

#[path = "svg_rasterize_html_font_selected_face_acquisition.rs"]
mod acquisition;
#[path = "svg_rasterize_html_font_selected_face_lifecycle.rs"]
mod lifecycle;
#[path = "svg_rasterize_html_font_selected_seal.rs"]
mod seal;
#[path = "svg_rasterize_html_font_selected_face_state.rs"]
mod state;
use state::State;

const MAX_REGISTERED_FACES: usize = 512;

pub(super) struct Context {
    pub(super) original: Arc<Database>,
    pub(super) proof: Arc<CreationProof>,
    pub(super) text: String,
    state: Mutex<Option<State>>,
}

fn reused_alias_id(
    alias: &seal::Alias,
    face: &resvg::usvg::fontdb::FaceInfo,
    database: &Arc<Database>,
) -> Option<ID> {
    let matched = alias.matches(face, database);
    #[cfg(test)]
    if matched {
        record_alias("reused", alias.origin, alias.face.id);
    }
    matched.then_some(alias.face.id)
}

pub(super) struct Attempt<'a> {
    context: &'a Context,
    #[cfg(test)]
    _observation: super::super::raster_observer::AttemptScope,
}

struct AdmittedAttempt<'a> {
    context: &'a Context,
    state: MutexGuard<'a, Option<State>>,
    admission: super::super::admission::Admission<'a>,
}

impl Context {
    pub(super) fn new(
        original: Arc<Database>,
        proof: Arc<CreationProof>,
        text: String,
    ) -> Option<Arc<Self>> {
        #[cfg(test)]
        let (original, proof) =
            super::super::raster_observer::local_missing_font_fixture(original, proof);
        /* WHY: ここでは File metadata のみを保持し、全 uncached 検証は attempt 開始時に行う。 */
        if !proof.seal_original(&original) || original.len() > MAX_REGISTERED_FACES {
            return None;
        }
        Some(Arc::new(Self {
            original,
            proof,
            text,
            state: Mutex::new(None),
        }))
    }

    pub(super) fn install(
        self: &Arc<Self>,
        options: &mut usvg::Options<'static>,
        valid: Arc<AtomicBool>,
        selected: Arc<AtomicBool>,
    ) {
        let selector = std::mem::replace(
            &mut options.font_resolver.select_font,
            usvg::FontResolver::default_font_selector(),
        );
        let context = Arc::clone(self);
        let font_valid = Arc::clone(&valid);
        options.font_resolver.select_font = Box::new(move |font, database| {
            /* WHY: alias suffix は family query と元 ID の admission に参加させない。 */
            let mut original = Arc::clone(&context.original);
            let origin = selector(font, &mut original);
            selected.store(true, Ordering::Relaxed);
            let (actual, admitted) = context.admitted_face(font, database, &original, origin);
            if !admitted {
                font_valid.store(false, Ordering::Relaxed);
            }
            #[cfg(test)]
            super::super::raster_observer::known_selector(font, database, actual, admitted);
            actual
        });
        super::install_fallback(options, valid);
    }

    fn admitted_face(
        &self,
        font: &usvg::Font,
        database: &mut Arc<Database>,
        original: &Arc<Database>,
        origin: Option<ID>,
    ) -> (Option<ID>, bool) {
        if !Arc::ptr_eq(original, &self.original) {
            return (origin.filter(|id| database.face(*id).is_some()), false);
        }
        let Ok(state) = self.state.lock() else {
            return (None, false);
        };
        if state.is_none() {
            return (None, false);
        }
        self.render_admitted_face(font, database, origin, state)
    }

    fn render_face(
        &self,
        database: &mut Arc<Database>,
        origin: ID,
        admitted: Option<AdmittedAttempt<'_>>,
    ) -> Option<ID> {
        let (mut locked, admission) = match admitted {
            Some(receipt) => {
                if !std::ptr::eq(receipt.context, self) {
                    return None;
                }
                (receipt.state, Some(receipt.admission))
            }
            None => (self.state.lock().ok()?, None),
        };
        let state = locked.as_mut()?;
        /* WHY: selected_shared_face が直後の取得前後で同じ original proof を検証する。 */
        if !seal::database_current(&self.original, database, &state.aliases) {
            return None;
        }
        self.render_selected_face(database, origin, state, admission.as_ref())
    }
}

#[cfg(test)]
fn record_alias(event: &str, origin: ID, alias: ID) {
    use super::super::raster_observer::{Metric, count};
    count(if event == "appended" {
        Metric::AliasAppend
    } else {
        Metric::AliasReuse
    });
    super::super::raster_observer::record(|| {
        serde_json::json!({
            "stage": "selected_alias", "event": event,
            "originalLocalId": format!("{origin:?}"), "aliasLocalId": format!("{alias:?}")
        })
    });
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_selected_face_tests.rs"]
mod tests;
