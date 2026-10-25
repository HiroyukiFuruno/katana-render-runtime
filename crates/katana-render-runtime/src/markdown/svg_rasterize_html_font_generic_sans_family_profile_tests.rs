use super::super::{HtmlFontRequest, sources};
use crate::markdown::svg_rasterize::{SvgRasterizeOps, text_shaping};
use resvg::usvg::fontdb::{Database, Family, ID};
use std::sync::Arc;

const TEXT: &str = "KatanA renderer context";

struct ExpectedArialStyle {
    weight: u16,
    italic: bool,
    post_script_name: &'static str,
}

const EXPECTED_ARIAL_STYLES: &[ExpectedArialStyle] = &[
    ExpectedArialStyle {
        weight: 400,
        italic: false,
        post_script_name: "ArialMT",
    },
    ExpectedArialStyle {
        weight: 700,
        italic: false,
        post_script_name: "Arial-BoldMT",
    },
    ExpectedArialStyle {
        weight: 400,
        italic: true,
        post_script_name: "Arial-ItalicMT",
    },
    ExpectedArialStyle {
        weight: 700,
        italic: true,
        post_script_name: "Arial-BoldItalicMT",
    },
];

#[test]
fn unknown_open_sans_falls_back_to_arial_for_all_requested_styles() -> Result<(), String> {
    let request = HtmlFontRequest::from_text("Open Sans, sans-serif", TEXT);
    let (database, proof) = sources::build_known_html_font_db(&request);
    let database = Arc::new(database);
    assert_eq!(database.family_name(&Family::SansSerif), "Arial");
    assert!(text_shaping::known_matching(&database, "Open Sans", 400, false).is_none());
    for style in EXPECTED_ARIAL_STYLES {
        assert_open_sans_fallback_style(&database, &proof, style)?;
    }
    assert_explicit_family("Helvetica")?;
    assert_explicit_family("Trebuchet MS")?;
    Ok(())
}

fn assert_open_sans_fallback_style(
    database: &Arc<Database>,
    proof: &super::CreationProof,
    style: &ExpectedArialStyle,
) -> Result<(), String> {
    let selected = selected_face(database, "Open Sans, sans-serif", style)?;
    let generic = selected_face(database, "sans-serif", style)?;
    assert_eq!(selected, generic);
    assert!(admit_in_scope(database, proof, "Open Sans, sans-serif", style).is_none());
    let admitted = admit_in_scope(database, proof, "sans-serif, Open Sans", style)
        .ok_or("known font admission rejected generic Arial face")?;
    assert_eq!(selected, admitted);
    let face = database
        .face(selected)
        .ok_or("selected Arial face missing")?;
    assert_eq!(face.post_script_name, style.post_script_name);
    Ok(())
}

fn admit_in_scope(
    database: &Arc<Database>,
    proof: &super::CreationProof,
    family: &str,
    style: &ExpectedArialStyle,
) -> Option<ID> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        super::admit(database, proof, family, TEXT, style.weight, style.italic)
    })
}

fn assert_explicit_family(family: &str) -> Result<(), String> {
    let request = HtmlFontRequest::from_text(family, TEXT);
    let (database, _) = sources::build_known_html_font_db(&request);
    let style = &EXPECTED_ARIAL_STYLES[0];
    let selected = selected_face(&database, family, style)?;
    let face = database
        .face(selected)
        .ok_or("explicit family face missing")?;
    if face
        .families
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case(family))
    {
        Ok(())
    } else {
        Err(format!("explicit family {family} selected another face"))
    }
}

fn selected_face(
    database: &Database,
    family: &str,
    style: &ExpectedArialStyle,
) -> Result<ID, String> {
    text_shaping::known_matching(database, family, style.weight, style.italic)
        .ok_or_else(|| format!("font face for {family} is missing"))
}
