use super::{GeneratedHtmlTextProvenance, HtmlTextRouting, tree_ids};
use resvg::usvg::{Options, Tree};
use std::{collections::HashSet, sync::Arc};

fn parse(svg: &str) -> Result<Tree, String> {
    let font_bytes = crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES.to_vec();
    let mut fontdb = resvg::usvg::fontdb::Database::new();
    fontdb.load_font_data(font_bytes);
    fontdb.set_sans_serif_family("Noto Sans");
    let options = Options {
        font_family: "Noto Sans".to_string(),
        fontdb: Arc::new(fontdb),
        ..Options::default()
    };
    Tree::from_str(svg, &options).map_err(|error| error.to_string())
}

fn recorded_markup(provenance: &mut GeneratedHtmlTextProvenance) -> String {
    provenance.record(
        r#"<text x="1" y="12" font-family="Noto Sans" fill="black">Label</text>"#.to_string(),
    )
}

fn wrap(markup: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="24">{markup}</svg>"#)
}

fn validated(
    provenance: &GeneratedHtmlTextProvenance,
    svg: &str,
) -> Result<HashSet<String>, String> {
    routing(provenance, svg).map(|routing| routing.ids)
}

fn routing(provenance: &GeneratedHtmlTextProvenance, svg: &str) -> Result<HtmlTextRouting, String> {
    let tree = parse(svg)?;
    let document = resvg::usvg::roxmltree::Document::parse(svg);
    let shared = provenance
        .validated_routing_with_document(&document, &tree)
        .map_err(|error| error.to_string())?;
    let original = provenance
        .validated_routing(svg, &tree)
        .map_err(|error| error.to_string())?;
    assert_eq!(shared.ids, original.ids);
    assert_eq!(shared.all_text_verified, original.all_text_verified);
    Ok(shared)
}

fn embedded_text_image(id: Option<&str>) -> String {
    let id_attribute = id.map_or_else(String::new, |value| format!("id='{value}'%20"));
    format!(
        r#"<image id="embedded" width="1" height="1" href="data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20width='1'%20height='1'%3E%3Ctext%20{id_attribute}font-family='Noto%20Sans'%20x='0'%20y='1'%3EX%3C/text%3E%3C/svg%3E"/>"#
    )
}

#[test]
fn exact_generated_text_markup_is_registered() -> Result<(), String> {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let markup = recorded_markup(&mut provenance);
    let svg = wrap(&markup);
    assert!(markup.starts_with("<text x=\"1\""));
    assert_eq!(
        validated(&provenance, &svg)?,
        HashSet::from(["krr-html-text-1".to_string()])
    );
    assert!(routing(&provenance, &svg)?.all_text_verified);
    Ok(())
}

#[test]
fn changed_markup_or_moved_id_is_not_registered() -> Result<(), String> {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let markup = recorded_markup(&mut provenance);
    let changed = wrap(&markup.replace("fill=\"black\"", "fill=\"red\""));
    let id = "krr-html-text-1";
    let text = markup.replace(&format!(" id=\"{id}\""), "");
    let moved = wrap(&format!("<g id=\"{id}\">{text}</g>"));
    for svg in [changed, moved] {
        assert!(validated(&provenance, &svg)?.is_empty());
        assert!(!routing(&provenance, &svg)?.all_text_verified);
    }
    Ok(())
}

#[test]
fn duplicate_source_id_disables_generated_text_routing() -> Result<(), String> {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let markup = recorded_markup(&mut provenance);
    let svg = wrap(&format!(r#"{markup}<path id="krr-html-text-1" d="M0 0"/>"#));
    assert!(validated(&provenance, &svg)?.is_empty());
    assert!(!routing(&provenance, &svg)?.all_text_verified);
    Ok(())
}

#[test]
fn embedded_svg_text_duplicate_rejects_only_colliding_registration() -> Result<(), String> {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let markup = recorded_markup(&mut provenance);
    let nested = r#"<image id="embedded" width="1" height="1" href="data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20width='1'%20height='1'%3E%3Ctext%20id='krr-html-text-1'%20font-family='Noto%20Sans'%20x='0'%20y='1'%3EX%3C/text%3E%3C/svg%3E"/>"#;
    let svg = wrap(&format!("{nested}{markup}"));
    let tree = parse(&svg)?;
    let ids = tree_ids(&tree);
    assert_eq!(ids.counts["krr-html-text-1"].len(), 2);
    assert!(
        provenance
            .validated_ids(&svg, &tree)
            .map_err(|error| error.to_string())?
            .is_empty()
    );
    Ok(())
}

#[test]
fn embedded_svg_text_without_id_disables_complete_routing() -> Result<(), String> {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let markup = recorded_markup(&mut provenance);
    let svg = wrap(&format!("{}{markup}", embedded_text_image(None)));
    let routing = routing(&provenance, &svg)?;
    assert_eq!(routing.ids, HashSet::from(["krr-html-text-1".to_string()]));
    assert!(!routing.all_text_verified);
    Ok(())
}

#[test]
fn embedded_svg_text_with_foreign_id_disables_complete_routing() -> Result<(), String> {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let markup = recorded_markup(&mut provenance);
    let svg = wrap(&format!("{}{markup}", embedded_text_image(Some("foreign"))));
    let routing = routing(&provenance, &svg)?;
    assert_eq!(routing.ids, HashSet::from(["krr-html-text-1".to_string()]));
    assert!(!routing.all_text_verified);
    Ok(())
}

#[test]
fn empty_provenance_never_claims_complete_text_routing() -> Result<(), String> {
    let provenance = GeneratedHtmlTextProvenance::default();
    let svg = wrap(r#"<text font-family="Noto Sans" x="1" y="12">Label</text>"#);
    let routing = routing(&provenance, &svg)?;
    assert!(routing.ids.is_empty());
    assert!(!routing.all_text_verified);
    Ok(())
}

#[test]
fn unrelated_duplicate_ids_do_not_disable_generated_text_routing() -> Result<(), String> {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let markup = recorded_markup(&mut provenance);
    let duplicate_paths = r#"<path id="shared" d="M0 0"/><path id="shared" d="M1 1"/>"#;
    let svg = wrap(&format!("{markup}{duplicate_paths}"));
    assert_eq!(
        validated(&provenance, &svg)?,
        HashSet::from(["krr-html-text-1".to_string()])
    );
    Ok(())
}

#[test]
fn partial_generated_text_registration_disables_complete_routing() -> Result<(), String> {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let first = recorded_markup(&mut provenance);
    let second = recorded_markup(&mut provenance);
    let changed = second.replace("Label", "Changed");
    let svg = wrap(&format!("{first}{changed}"));
    let routing = routing(&provenance, &svg)?;
    assert_eq!(routing.ids, HashSet::from(["krr-html-text-1".to_string()]));
    assert!(!routing.all_text_verified);
    Ok(())
}

#[test]
fn malformed_source_is_rejected_when_provenance_exists() -> Result<(), String> {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let _ = recorded_markup(&mut provenance);
    let tree = parse(r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="24"/>"#)?;
    assert!(provenance.validated_ids("<svg>", &tree).is_err());
    Ok(())
}

#[test]
fn shared_document_hash_ranges_follow_original_utf8_source() -> Result<(), String> {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let markup = recorded_markup(&mut provenance);
    let svg = wrap(&format!("<!-- 日本語の前置き -->{markup}"));
    assert!(routing(&provenance, &svg)?.all_text_verified);
    Ok(())
}
