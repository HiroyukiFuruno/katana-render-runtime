use super::{legacy_options, selected_face, text_shaping};
use resvg::usvg;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) fn known_tree(
    svg: &str,
    options: &usvg::Options<'_>,
    valid: &AtomicBool,
    selected: &AtomicBool,
    context: Option<&Arc<selected_face::Context>>,
    #[cfg(test)] timing: &super::super::raster_observer::TreeParseTiming,
) -> Result<usvg::Tree, usvg::Error> {
    text_shaping::with_validated_tree_parse(|| {
        reset_known_attempt(valid, selected);
        let legacy = context
            .filter(|_| !text_shaping::known_memo_usable())
            .and_then(|context| legacy_options(context));
        let attempt = begin_selected_attempt(context, legacy.is_none());
        let active_options = legacy.as_ref().map_or(options, |options| &options.0);
        #[cfg(test)]
        let result = timing.tree(svg, active_options);
        #[cfg(not(test))]
        let result = usvg::Tree::from_str(svg, active_options);
        if legacy.is_none() {
            finish_selected_attempt(context, attempt.as_ref(), &result, valid);
        }
        if let Some((_, legacy_valid, legacy_selected)) = &legacy {
            valid.store(legacy_valid.load(Ordering::Relaxed), Ordering::Relaxed);
            selected.store(legacy_selected.load(Ordering::Relaxed), Ordering::Relaxed);
        }
        drop(attempt);
        result
    })
}

fn finish_selected_attempt(
    context: Option<&Arc<selected_face::Context>>,
    attempt: Option<&selected_face::Attempt<'_>>,
    result: &Result<usvg::Tree, usvg::Error>,
    valid: &AtomicBool,
) {
    if let Some(context) = context
        && (attempt.is_none()
            || result
                .as_ref()
                .is_ok_and(|tree| !context.finish(tree.fontdb())))
    {
        valid.store(false, Ordering::Relaxed);
    }
}

fn reset_known_attempt(valid: &AtomicBool, selected: &AtomicBool) {
    valid.store(true, Ordering::Relaxed);
    selected.store(false, Ordering::Relaxed);
    #[cfg(test)]
    super::super::raster_observer::known_attempt();
}

fn begin_selected_attempt<'a>(
    context: Option<&'a Arc<selected_face::Context>>,
    selected_route: bool,
) -> Option<selected_face::Attempt<'a>> {
    if selected_route {
        context.and_then(|context| context.begin())
    } else {
        None
    }
}
