use super::{Metric, TreeParseTiming, count, record};
use resvg::usvg::fontdb::{Database, ID, Source};
use std::sync::atomic::{AtomicBool, Ordering};

#[path = "svg_rasterize_html_font_raster_observer_styled.rs"]
mod styled;

fn face(database: &Database, id: Option<ID>) -> serde_json::Value {
    let Some(face) = id.and_then(|id| database.face(id)) else {
        return serde_json::Value::Null;
    };
    let (kind, path) = match &face.source {
        Source::File(path) => ("File", Some(path.display().to_string())),
        Source::SharedFile(path, _) => ("SharedFile", Some(path.display().to_string())),
        Source::Binary(_) => ("Binary", None),
    };
    serde_json::json!({
        "databaseFaceIdLocalOnly": format!("{:?}", face.id), "sourceKind": kind,
        "sourcePath": path, "faceIndex": face.index,
        "families": face.families.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        "postScriptName": face.post_script_name, "weight": face.weight.0,
        "style": format!("{:?}", face.style), "stretch": format!("{:?}", face.stretch),
        "monospaced": face.monospaced,
    })
}

pub(in super::super) fn install_eager(options: &mut resvg::usvg::Options<'static>) {
    install_eager_selector(options);
    install_eager_fallback(options);
}

fn install_eager_selector(options: &mut resvg::usvg::Options<'static>) {
    let selector = std::mem::replace(
        &mut options.font_resolver.select_font,
        resvg::usvg::FontResolver::default_font_selector(),
    );
    options.font_resolver.select_font = Box::new(move |font, database| {
        let actual = selector(font, database);
        count(Metric::EagerSelector);
        super::super::lazy_diagnostic::record_actual_selected_face(database, actual);
        record(|| {
            serde_json::json!({
                "stage": "selector", "route": "eager", "firstFamily": format!("{:?}", font.families().first()),
                "families": font.families().iter().map(|family| format!("{family:?}")).collect::<Vec<_>>(),
                "weight": font.weight(), "style": format!("{:?}", font.style()),
                "stretch": format!("{:?}", font.stretch()), "actualFace": face(database, actual),
            })
        });
        actual
    });
}

fn install_eager_fallback(options: &mut resvg::usvg::Options<'static>) {
    styled::install_eager_styled_fallback(options);
    let fallback = std::mem::replace(
        &mut options.font_resolver.select_fallback,
        resvg::usvg::FontResolver::default_fallback_selector(),
    );
    options.font_resolver.select_fallback = Box::new(move |character, excluded, database| {
        let actual = fallback(character, excluded, database);
        count(Metric::EagerFallback);
        super::super::lazy_diagnostic::record_actual_selected_face(database, actual);
        record(|| {
            serde_json::json!({"stage": "fallback", "route": "eager",
            "character": character.to_string(), "codepoint": character as u32,
            "excludedLocalIds": excluded.iter().map(|id| format!("{id:?}")).collect::<Vec<_>>(),
            "actualFace": face(database, actual)})
        });
        actual
    });
}

pub(in super::super) fn known_result(
    success: bool,
    valid: &AtomicBool,
    selected: &AtomicBool,
    timing: &TreeParseTiming,
) {
    record(
        || serde_json::json!({"stage": "known_parse_result", "success": success, "valid": valid.load(Ordering::Relaxed), "selected": selected.load(Ordering::Relaxed), "treeParseNs": timing.elapsed_ns.get()}),
    );
}

pub(in super::super) fn eager_reparse(valid: &AtomicBool, selected: &AtomicBool) {
    record(
        || serde_json::json!({"stage": "parse_route", "route": "eager_reparse", "valid": valid.load(Ordering::Relaxed), "selected": selected.load(Ordering::Relaxed)}),
    );
}

pub(in super::super) fn known_selector(
    font: &resvg::usvg::Font,
    database: &Database,
    actual: Option<ID>,
    admitted: bool,
) {
    count(Metric::KnownSelector);
    super::super::lazy_diagnostic::record_actual_selected_face(database, actual);
    record(|| {
        serde_json::json!({
            "stage": "selector", "route": "known_probe", "firstFamily": format!("{:?}", font.families().first()),
            "families": font.families().iter().map(|family| format!("{family:?}")).collect::<Vec<_>>(),
            "weight": font.weight(), "style": format!("{:?}", font.style()),
            "stretch": format!("{:?}", font.stretch()), "admitted": admitted, "actualFace": face(database, actual),
        })
    });
    super::local_missing_after_selector(database, admitted);
}

pub(in super::super) fn known_fallback(
    character: char,
    excluded: &[ID],
    database: &Database,
    actual: Option<ID>,
) {
    count(Metric::KnownFallback);
    super::super::lazy_diagnostic::record_actual_selected_face(database, actual);
    record(
        || serde_json::json!({"stage": "fallback", "route": "known_probe", "character": character.to_string(), "codepoint": character as u32, "excludedLocalIds": excluded.iter().map(|id| format!("{id:?}")).collect::<Vec<_>>(), "actualFace": face(database, actual)}),
    );
}

pub(in super::super) fn admission_result(
    database: &Database,
    expected: Option<ID>,
    actual: Option<ID>,
) {
    record(|| {
        serde_json::json!({
            "stage": "admission_result", "reason": if expected.is_none() { "admission_returned_none_inner_reason_unobserved" } else if actual != expected { "actual_face_mismatch" } else { "accepted" },
            "expectedFace": face(database, expected),
        })
    });
}

pub(in super::super) fn prepare_unavailable() {
    record(|| serde_json::json!({"stage": "parse_route", "route": "eager_prepare_unavailable"}));
}

pub(in super::super) fn known_return() {
    record(|| serde_json::json!({"stage": "parse_route", "route": "known_return"}));
}

pub(in super::super) fn known_attempt() {
    record(|| serde_json::json!({"stage": "known_parse_attempt"}));
}

pub(in super::super) fn eager_attempt() {
    record(|| serde_json::json!({"stage": "eager_parse_attempt"}));
}

pub(in super::super) fn reject_family() {
    record(
        || serde_json::json!({"stage": "admission_rejection", "reason": "first_family_not_named_or_missing"}),
    );
}

pub(in super::super) fn reject_stretch() {
    record(|| serde_json::json!({"stage": "admission_rejection", "reason": "stretch_not_normal"}));
}

pub(in super::super) fn reject_style() {
    record(
        || serde_json::json!({"stage": "admission_rejection", "reason": "style_not_normal_or_italic"}),
    );
}

pub(in super::super) fn prepare_ascii_unavailable() {
    record(
        || serde_json::json!({"stage": "prepare", "success": false, "reason": "decoded_ascii_unavailable"}),
    );
}

pub(in super::super) fn prepare_known_unavailable() {
    record(
        || serde_json::json!({"stage": "prepare", "success": false, "reason": "known_for_unavailable"}),
    );
}

pub(in super::super) fn prepare_ready() {
    record(|| serde_json::json!({"stage": "prepare", "success": true}));
}

pub(in super::super) fn eager_result(success: bool) {
    record(|| serde_json::json!({"stage": "eager_parse_result", "success": success}));
}
