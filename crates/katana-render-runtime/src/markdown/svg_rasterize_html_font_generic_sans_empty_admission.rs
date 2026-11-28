use super::super::super::super as html;
use super::super::super::admit;
use super::face_assertions::{assert_same_face, metrics};
use crate::markdown::svg_rasterize::{SvgRasterizeOps, text_shaping};
use html::proof::CreationProof;
use html::{DiscoveryMode, HtmlFontRequest, decision_observer, sources};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

const NORMAL_FONT_WEIGHT: u16 = 400;
const INEXACT_FONT_WEIGHT: u16 = 500;

fn empty_admitted_ids(
    family: &str,
    known: &Arc<Database>,
    proof: &CreationProof,
    eager: &Database,
    weight: u16,
    italic: bool,
) -> Result<(ID, ID), String> {
    let known_id = SvgRasterizeOps::with_html_font_render_scope(|| {
        admit(known, proof, family, "", weight, italic)
    })
    .ok_or("empty exact Arial admission failed")?;
    let eager_id = text_shaping::known_matching(eager, family, weight, italic)
        .ok_or("eager empty-text Arial query failed")?;
    assert_same_face(known, known_id, eager, eager_id)?;
    assert_eq!(metrics(known, known_id)?, metrics(eager, eager_id)?);
    assert!(proof.contains(known_id) && proof.current(known));
    Ok((known_id, eager_id))
}

pub(in super::super) fn assert_empty_style(
    observer: &decision_observer::Scope,
    family: &str,
    request: &HtmlFontRequest,
    known: &Arc<Database>,
    proof: &CreationProof,
    eager: &Database,
    shape: (u16, bool),
) -> Result<(), String> {
    let (weight, italic) = shape;
    let (_known_id, eager_id) = empty_admitted_ids(family, known, proof, eager, weight, italic)?;
    let measured = SvgRasterizeOps::with_html_font_render_scope(|| {
        html::measured_database(family, "", weight, italic)
    });
    assert_eq!(measured.key.request, *request);
    assert_eq!(measured.key.discovery, DiscoveryMode::KnownOnly);
    let measured_id = text_shaping::known_matching(&measured.database, family, weight, italic)
        .ok_or("measured empty-text Arial face missing")?;
    assert_same_face(eager, eager_id, &measured.database, measured_id)?;
    assert_eq!(
        metrics(&measured.database, measured_id)?,
        metrics(eager, eager_id)?
    );
    assert_empty_decision(observer, family, request, weight, italic)?;
    Ok(())
}

fn assert_empty_decision(
    observer: &decision_observer::Scope,
    family: &str,
    request: &HtmlFontRequest,
    weight: u16,
    italic: bool,
) -> Result<(), String> {
    let record = observer
        .records()
        .into_iter()
        .find(|record| {
            record.family == family
                && record.text.is_empty()
                && record.weight == weight
                && record.italic == italic
        })
        .ok_or("empty measured lookup was not observed")?;
    assert_eq!(record.key.request, *request);
    assert_eq!(record.key.discovery, DiscoveryMode::KnownOnly);
    Ok(())
}

fn assert_empty_family_rejections(database: &Arc<Database>, proof: &CreationProof) {
    for family in [
        "serif",
        "monospace",
        "cursive",
        "fantasy",
        "system-ui",
        "serif, Arial",
        "KRR missing family, sans-serif",
        "KRR missing family",
    ] {
        assert!(
            SvgRasterizeOps::with_html_font_render_scope(|| admit(
                database,
                proof,
                family,
                "",
                NORMAL_FONT_WEIGHT,
                false
            ))
            .is_none(),
            "unsupported empty-text family must stay rejected: {family}"
        );
    }
}

pub(in super::super) fn assert_empty_admission_rejections() {
    let request = HtmlFontRequest::from_text("sans-serif", "");
    let (database, proof) = sources::build_known_html_font_db(&request);
    let database = Arc::new(database);
    assert_empty_family_rejections(&database, &proof);
    for (weight, italic) in [(INEXACT_FONT_WEIGHT, false), (INEXACT_FONT_WEIGHT, true)] {
        assert!(
            SvgRasterizeOps::with_html_font_render_scope(|| admit(
                &database,
                &proof,
                "sans-serif",
                "",
                weight,
                italic
            ))
            .is_none(),
            "unsupported empty-text style must stay rejected: {weight}/{italic}"
        );
    }
}
