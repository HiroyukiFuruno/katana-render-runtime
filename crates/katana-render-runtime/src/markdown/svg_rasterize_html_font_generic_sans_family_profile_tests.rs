use super::super::{HtmlFontRequest, sources};
use crate::markdown::svg_rasterize::{SvgRasterizeOps, text_shaping};
use resvg::usvg::fontdb::{Database, Family, ID};
use std::sync::Arc;

const TEXT: &str = "KatanA renderer context";

#[test]
fn devanagari_request_uses_the_known_unicode_fallback_route() {
    let request = HtmlFontRequest::from_text("sans-serif", "अ");
    assert!(request.needs_unicode_fallback);
    let (database, proof) = sources::build_known_html_font_db(&request);
    assert!(proof.current(&database));
}

struct ExpectedGenericSansStyle {
    weight: u16,
    italic: bool,
    mac_post_script_name: &'static str,
    other_post_script_name: &'static str,
}

const EXPECTED_GENERIC_SANS_STYLES: &[ExpectedGenericSansStyle] = &[
    ExpectedGenericSansStyle {
        weight: 400,
        italic: false,
        mac_post_script_name: "Helvetica",
        other_post_script_name: "ArialMT",
    },
    ExpectedGenericSansStyle {
        weight: 700,
        italic: false,
        mac_post_script_name: "Helvetica-Bold",
        other_post_script_name: "Arial-BoldMT",
    },
    ExpectedGenericSansStyle {
        weight: 400,
        italic: true,
        mac_post_script_name: "Helvetica-Oblique",
        other_post_script_name: "Arial-ItalicMT",
    },
    ExpectedGenericSansStyle {
        weight: 700,
        italic: true,
        mac_post_script_name: "Helvetica-BoldOblique",
        other_post_script_name: "Arial-BoldItalicMT",
    },
];

#[test]
fn unknown_open_sans_falls_back_to_configured_sans_for_all_requested_styles() -> Result<(), String>
{
    let request = HtmlFontRequest::from_text("Open Sans, sans-serif", TEXT);
    let (database, proof) = sources::build_known_html_font_db(&request);
    let database = Arc::new(database);
    let expected_family = configured_sans_family();
    assert_eq!(database.family_name(&Family::SansSerif), expected_family);
    assert!(text_shaping::known_matching(&database, "Open Sans", 400, false).is_none());
    for style in configured_sans_styles() {
        assert_open_sans_fallback_style(&database, &proof, style)?;
    }
    assert_explicit_family("Helvetica")?;
    assert_explicit_family("Arial")?;
    assert_explicit_family("Trebuchet MS")?;
    Ok(())
}

fn assert_open_sans_fallback_style(
    database: &Arc<Database>,
    proof: &super::CreationProof,
    style: &ExpectedGenericSansStyle,
) -> Result<(), String> {
    let selected = selected_face(database, "Open Sans, sans-serif", style)?;
    let generic = selected_face(database, "sans-serif", style)?;
    assert_eq!(selected, generic);
    assert!(admit_in_scope(database, proof, "Open Sans, sans-serif", style).is_none());
    let admitted = admit_in_scope(database, proof, "sans-serif, Open Sans", style)
        .ok_or("known font admission rejected configured generic sans face")?;
    assert_eq!(selected, admitted);
    let face = database
        .face(selected)
        .ok_or("selected generic sans face missing")?;
    assert_eq!(face.post_script_name, expected_post_script_name(style));
    Ok(())
}

fn admit_in_scope(
    database: &Arc<Database>,
    proof: &super::CreationProof,
    family: &str,
    style: &ExpectedGenericSansStyle,
) -> Option<ID> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        super::admit(database, proof, family, TEXT, style.weight, style.italic)
    })
}

fn assert_explicit_family(family: &str) -> Result<(), String> {
    let request = HtmlFontRequest::from_text(family, TEXT);
    let (database, _) = sources::build_known_html_font_db(&request);
    let style = &EXPECTED_GENERIC_SANS_STYLES[0];
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
    style: &ExpectedGenericSansStyle,
) -> Result<ID, String> {
    text_shaping::known_matching(database, family, style.weight, style.italic)
        .ok_or_else(|| format!("font face for {family} is missing"))
}

fn configured_sans_family() -> &'static str {
    if cfg!(target_os = "macos") {
        "Helvetica"
    } else {
        "Arial"
    }
}

fn configured_sans_styles() -> &'static [ExpectedGenericSansStyle] {
    EXPECTED_GENERIC_SANS_STYLES
}

fn expected_post_script_name(style: &ExpectedGenericSansStyle) -> &'static str {
    if cfg!(target_os = "macos") {
        style.mac_post_script_name
    } else {
        style.other_post_script_name
    }
}
